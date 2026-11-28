//! Tests of the rendezvous protocol of the symmetric stream runtime (see
//! `wit/stream.wit`) through its raw interface: both orders of arrival, partial
//! transfers, cancellation, EOF and use from several threads.

use wit_bindgen_symmetric_rt::symmetric_stream::{Address, Buffer, StreamObj, StreamState};
use wit_bindgen_symmetric_rt::EventGenerator;

fn new_stream() -> (StreamObj, StreamObj) {
    // the stream runtime needs the executor library, make sure it gets linked
    drop(std::hint::black_box(EventGenerator::new()));
    let writer = StreamObj::new();
    let reader = writer.clone(false);
    (writer, reader)
}

fn buffer(data: &[u64], size: u64) -> Buffer {
    let address = unsafe { Address::from_handle(data.as_ptr() as usize) };
    let buffer = Buffer::new(address, data.len() as u64, 8);
    buffer.set_size(size);
    buffer
}

fn count(result: Result<Buffer, StreamState>) -> u64 {
    result
        .unwrap_or_else(|_| panic!("not completed"))
        .get_size()
}

fn is_pending(result: Result<Buffer, StreamState>) -> bool {
    matches!(result, Err(StreamState::Pending))
}

#[test]
fn reader_first() {
    let (writer, reader) = new_stream();
    let dest = [0u64; 4];
    reader.start_reading(buffer(&dest, 0)).unwrap();
    assert!(is_pending(reader.read_result()));
    let src = [7u64, 8];
    writer.offer_write(buffer(&src, 2)).unwrap();
    // the writer arrived second and copied
    assert_eq!(count(reader.read_result()), 2);
    assert_eq!(count(writer.write_result()), 2);
    assert_eq!(dest[..2], [7, 8]);
}

#[test]
fn writer_first_with_partial_consumption() {
    let (writer, reader) = new_stream();
    let src = [1u64, 2, 3, 4, 5];
    writer.offer_write(buffer(&src, 5)).unwrap();
    assert!(is_pending(writer.write_result()));

    // the reader is second, it copies what fits
    let small = [0u64; 2];
    reader.start_reading(buffer(&small, 0)).unwrap();
    assert_eq!(count(reader.read_result()), 2);
    assert_eq!(small, [1, 2]);
    // the write stays pending, the runtime remembers the offset
    assert!(is_pending(writer.write_result()));
    // a second write is still rejected
    assert!(writer.offer_write(buffer(&src, 5)).is_err());

    let big = [0u64; 8];
    reader.start_reading(buffer(&big, 0)).unwrap();
    // synchronously the rest of the same write
    assert_eq!(count(reader.read_result()), 3);
    assert_eq!(big[..3], [3, 4, 5]);
    // the write completes with the total
    assert_eq!(count(writer.write_result()), 5);
    assert!(is_pending(writer.write_result()));
}

#[test]
fn write_stays_pending_while_the_reader_reads_one_by_one() {
    let (writer, reader) = new_stream();
    let src = [10u64, 20];
    writer.offer_write(buffer(&src, 2)).unwrap();
    let one = [0u64; 1];
    reader.start_reading(buffer(&one, 0)).unwrap();
    assert_eq!(count(reader.read_result()), 1);
    assert_eq!(one[0], 10);
    assert!(is_pending(writer.write_result()));
    reader.start_reading(buffer(&one, 0)).unwrap();
    assert_eq!(count(reader.read_result()), 1);
    assert_eq!(one[0], 20);
    assert_eq!(count(writer.write_result()), 2);
}

#[test]
fn reader_first_partial_keeps_the_write_pending() {
    let (writer, reader) = new_stream();
    let src = [1u64, 2, 3, 4, 5];
    let small = [0u64; 2];
    reader.start_reading(buffer(&small, 0)).unwrap();
    writer.offer_write(buffer(&src, 5)).unwrap();
    // the reader completes with what fits, the write is not complete
    assert_eq!(count(reader.read_result()), 2);
    assert_eq!(small, [1, 2]);
    assert!(is_pending(writer.write_result()));
    let rest = [0u64; 2];
    reader.start_reading(buffer(&rest, 0)).unwrap();
    assert_eq!(count(reader.read_result()), 2);
    assert_eq!(rest, [3, 4]);
    assert!(is_pending(writer.write_result()));
    reader.start_reading(buffer(&rest, 0)).unwrap();
    assert_eq!(count(reader.read_result()), 1);
    assert_eq!(rest[0], 5);
    assert_eq!(count(writer.write_result()), 5);
}

#[test]
fn reader_first_whole_write_completes() {
    let (writer, reader) = new_stream();
    let src = [1u64, 2];
    let dest = [0u64; 2];
    reader.start_reading(buffer(&dest, 0)).unwrap();
    writer.offer_write(buffer(&src, 2)).unwrap();
    assert_eq!(count(reader.read_result()), 2);
    assert_eq!(count(writer.write_result()), 2);
}

#[test]
fn write_reports_the_reader_gone_when_observed() {
    // everything was consumed, but the reader is gone when the writer looks
    let (writer, reader) = new_stream();
    let src = [1u64, 2];
    writer.offer_write(buffer(&src, 2)).unwrap();
    let dest = [0u64; 2];
    reader.start_reading(buffer(&dest, 0)).unwrap();
    assert_eq!(count(reader.read_result()), 2);
    assert!(!writer.is_read_closed());
    drop(reader);
    // the count is the total, `is-read-closed` tells that the reader is gone
    assert_eq!(count(writer.write_result()), 2);
    assert!(writer.is_read_closed());
    assert!(matches!(writer.write_result(), Err(StreamState::Eof)));
}

#[test]
fn write_reports_progress_when_the_reader_is_dropped() {
    let (writer, reader) = new_stream();
    let src = [1u64, 2, 3];
    writer.offer_write(buffer(&src, 3)).unwrap();
    let dest = [0u64; 1];
    reader.start_reading(buffer(&dest, 0)).unwrap();
    assert_eq!(count(reader.read_result()), 1);
    assert!(is_pending(writer.write_result()));
    drop(reader);
    // the elements the reader took are gone, the others are the writer's
    assert_eq!(count(writer.write_result()), 1);
    assert!(writer.is_read_closed());
    assert!(matches!(writer.write_result(), Err(StreamState::Eof)));
    assert!(writer.cancel_write().is_none());
}

#[test]
fn cancel_write_reports_the_progress() {
    let (writer, reader) = new_stream();
    let src = [1u64, 2, 3, 4];
    writer.offer_write(buffer(&src, 4)).unwrap();
    let dest = [0u64; 3];
    reader.start_reading(buffer(&dest, 0)).unwrap();
    assert_eq!(count(reader.read_result()), 3);
    assert_eq!(writer.cancel_write().unwrap().get_size(), 3);
    // the fourth element is the writer's, nobody can read it
    reader.start_reading(buffer(&dest, 0)).unwrap();
    assert!(is_pending(reader.read_result()));
    assert!(writer.cancel_write().is_none());
}

#[test]
fn detached_write_is_consumed_piecewise() {
    let (writer, reader) = new_stream();
    {
        let src = [1u64, 2, 3];
        writer.write_detached(buffer(&src, 3));
    }
    drop(writer);
    let dest = [0u64; 2];
    reader.start_reading(buffer(&dest, 0)).unwrap();
    assert_eq!(count(reader.read_result()), 2);
    assert_eq!(dest, [1, 2]);
    reader.start_reading(buffer(&dest, 0)).unwrap();
    assert_eq!(count(reader.read_result()), 1);
    assert_eq!(dest[0], 3);
    assert!(matches!(reader.read_result(), Err(StreamState::Eof)));
}

#[test]
fn borrowed_buffer() {
    let (writer, reader) = new_stream();
    let dest = [0u64; 3];
    assert!(is_pending(writer.start_writing()));
    reader.start_reading(buffer(&dest, 0)).unwrap();
    let lent = writer.start_writing().unwrap();
    assert_eq!(lent.capacity(), 3);
    unsafe { *(lent.get_address().take_handle() as *mut u64) = 42 };
    lent.set_size(1);
    writer.finish_writing(lent).unwrap();
    assert_eq!(count(reader.read_result()), 1);
    assert_eq!(dest[0], 42);
}

#[test]
fn cancel_read() {
    let (writer, reader) = new_stream();
    let dest = [0u64; 2];
    assert!(reader.cancel_read().is_none());
    reader.start_reading(buffer(&dest, 0)).unwrap();
    // a second read is rejected
    assert!(reader.start_reading(buffer(&dest, 0)).is_err());
    let cancelled = reader.cancel_read().unwrap();
    assert_eq!(cancelled.get_size(), 0);
    // nothing references the buffer anymore, the writer has to wait
    assert!(is_pending(writer.start_writing()));
    let src = [5u64];
    writer.offer_write(buffer(&src, 1)).unwrap();
    assert!(is_pending(writer.write_result()));
    // a read which was completed meanwhile is not undone
    reader.start_reading(buffer(&dest, 0)).unwrap();
    assert_eq!(reader.cancel_read().unwrap().get_size(), 1);
    assert_eq!(dest[0], 5);
    assert_eq!(count(writer.write_result()), 1);
}

#[test]
fn cancel_write() {
    let (writer, reader) = new_stream();
    let src = [1u64, 2, 3];
    writer.offer_write(buffer(&src, 3)).unwrap();
    // a second write is rejected
    assert!(writer.offer_write(buffer(&src, 3)).is_err());
    assert_eq!(writer.cancel_write().unwrap().get_size(), 0);
    assert!(writer.cancel_write().is_none());
    // the offer is gone
    let dest = [0u64; 3];
    reader.start_reading(buffer(&dest, 0)).unwrap();
    assert!(is_pending(reader.read_result()));
    // the other side completed the write before it was cancelled
    writer.offer_write(buffer(&src, 3)).unwrap();
    assert_eq!(writer.cancel_write().unwrap().get_size(), 3);
    assert_eq!(count(reader.read_result()), 3);
    assert_eq!(dest, src);
}

#[test]
fn zero_length_read_does_not_consume_the_write() {
    let (writer, reader) = new_stream();
    let src = [1u64];
    writer.offer_write(buffer(&src, 1)).unwrap();
    let none: [u64; 0] = [];
    reader.start_reading(buffer(&none, 0)).unwrap();
    assert_eq!(count(reader.read_result()), 0);
    assert!(is_pending(writer.write_result()));
    let dest = [0u64; 1];
    reader.start_reading(buffer(&dest, 0)).unwrap();
    assert_eq!(count(reader.read_result()), 1);
    assert_eq!(count(writer.write_result()), 1);
}

#[test]
fn eof_after_writer_drop() {
    let (writer, reader) = new_stream();
    let dest = [0u64; 1];
    reader.start_reading(buffer(&dest, 0)).unwrap();
    drop(writer);
    assert!(matches!(reader.read_result(), Err(StreamState::Eof)));
    assert!(reader.is_write_closed());
    // the offer was withdrawn
    assert!(reader.cancel_read().is_none());
}

#[test]
fn data_before_eof() {
    let (writer, reader) = new_stream();
    let src = [9u64];
    writer.offer_write(buffer(&src, 1)).unwrap();
    let dest = [0u64; 1];
    reader.start_reading(buffer(&dest, 0)).unwrap();
    drop(writer);
    assert_eq!(count(reader.read_result()), 1);
    assert_eq!(dest[0], 9);
    assert!(matches!(reader.read_result(), Err(StreamState::Eof)));
}

#[test]
fn eof_after_reader_drop() {
    let (writer, reader) = new_stream();
    let src = [1u64];
    writer.offer_write(buffer(&src, 1)).unwrap();
    drop(reader);
    assert!(matches!(writer.write_result(), Err(StreamState::Eof)));
    assert!(writer.is_read_closed());
    assert!(matches!(writer.start_writing(), Err(StreamState::Eof)));
    assert!(writer.cancel_write().is_none());
}

#[test]
fn close_read_returns_the_buffer() {
    let (writer, reader) = new_stream();
    let dest = [0u64; 1];
    reader.start_reading(buffer(&dest, 0)).unwrap();
    assert_eq!(reader.close_read().unwrap().get_size(), 0);
    assert!(writer.is_read_closed());
    assert!(matches!(writer.start_writing(), Err(StreamState::Eof)));
}

#[test]
fn detached_write_survives_the_writer() {
    let (writer, reader) = new_stream();
    {
        let src = [77u64];
        writer.write_detached(buffer(&src, 1));
    }
    drop(writer);
    let dest = [0u64; 1];
    reader.start_reading(buffer(&dest, 0)).unwrap();
    assert_eq!(count(reader.read_result()), 1);
    assert_eq!(dest[0], 77);
    assert!(matches!(reader.read_result(), Err(StreamState::Eof)));
}

/// Readers and writers on different threads, in both orders of arrival,
/// with partial transfers. Every element arrives exactly once, in order.
#[test]
fn threads() {
    const TOTAL: u64 = 20_000;
    let (writer, reader) = new_stream();
    let (writer, reader) = (SendHandle(writer), SendHandle(reader));
    let producer = std::thread::spawn(move || {
        let writer = writer;
        let writer = &writer.0;
        let data: Vec<u64> = (0..TOTAL).collect();
        let mut next = 0usize;
        while next < data.len() {
            let chunk = ((next % 7) + 1).min(data.len() - next);
            let rest = &data[next..next + chunk];
            if next % 3 == 0 {
                // offer first (or complete a waiting read)
                writer.offer_write(buffer(rest, chunk as u64)).unwrap();
                next += loop {
                    match writer.write_result() {
                        Ok(done) => break done.get_size() as usize,
                        Err(StreamState::Pending) => std::thread::yield_now(),
                        Err(StreamState::Eof) => panic!("reader gone"),
                    }
                };
            } else {
                // use the buffer of the reader
                match writer.start_writing() {
                    Ok(lent) => {
                        let n = (lent.capacity() as usize).min(chunk);
                        let dest = lent.get_address().take_handle() as *mut u64;
                        unsafe { std::ptr::copy_nonoverlapping(rest.as_ptr(), dest, n) };
                        lent.set_size(n as u64);
                        writer.finish_writing(lent).unwrap();
                        next += n;
                    }
                    Err(StreamState::Pending) => std::thread::yield_now(),
                    Err(StreamState::Eof) => panic!("reader gone"),
                }
            }
        }
    });
    let consumer = std::thread::spawn(move || {
        let reader = reader;
        let reader = &reader.0;
        let mut expected = 0u64;
        let mut round = 0usize;
        while expected < TOTAL {
            let dest = vec![0u64; (round % 5) + 1];
            round += 1;
            reader.start_reading(buffer(&dest, 0)).unwrap();
            let done = loop {
                match reader.read_result() {
                    Ok(done) => break done,
                    Err(StreamState::Pending) => {
                        // sometimes give up (the writer may have completed meanwhile)
                        if round % 11 == 0 {
                            if let Some(done) = reader.cancel_read() {
                                // nothing, or what the writer completed meanwhile
                                break done;
                            }
                        }
                        std::thread::yield_now()
                    }
                    Err(StreamState::Eof) => panic!("writer gone"),
                }
            };
            for i in 0..done.get_size() as usize {
                assert_eq!(dest[i], expected);
                expected += 1;
            }
        }
    });
    producer.join().unwrap();
    consumer.join().unwrap();
}

struct SendHandle(StreamObj);
unsafe impl Send for SendHandle {}
