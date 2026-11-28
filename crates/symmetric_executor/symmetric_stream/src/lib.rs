//! Language neutral stream runtime of the symmetric ABI, implementing the
//! rendezvous protocol described at the top of `wit/stream.wit`.
//!
//! # State machine
//!
//! One [`State`] per stream, protected by a mutex (plus a condition variable
//! for the short "borrowed" section of the legacy `start-writing` protocol).
//! At most one read and one write operation are in flight. Each operation is
//! in exactly one of these states:
//!
//! ```text
//!   reader:  idle --start-reading--> offered (read_offer) --writer arrives--> done (read_done) --read-result--> idle
//!                                    offered --start-writing--> lent (read_lent) --finish-writing--> done
//!                                    offered/done --cancel-read--> idle
//!   writer:  idle --offer-write--> offered (write_offer) --reader arrives--> done (write_done) --write-result--> idle
//!                                  offered/done --cancel-write--> idle
//! ```
//!
//! If the counterpart is already offered when an operation starts, the
//! arriving side performs the transfer *inside that call* (under the lock, so a
//! concurrent cancel sees either the posted or the completed operation, never a
//! half copied one): `n = min(reader capacity, writer size)` elements are
//! copied bytewise, both operations move to *done* with count `n`, and both ready
//! events are activated. Ownership of those `n` lowered elements moves to the
//! reader. The rest of the writer's buffer (`[n, size)`) stays with the writer,
//! which learns about it from the count of its completed operation and offers
//! it again (or lifts it back). The runtime never remembers an offset: partial
//! consumption simply completes the operation of both sides, the remainder is
//! a new operation.
//!
//! Zero length operations: a zero length read/write (`n == 0` although the
//! other side is not empty) completes without consuming the counterpart
//! (it just signals readiness) and the counterpart stays offered.
//!
//! # Memory
//!
//! The runtime only dereferences the address of an offer while it is
//! `offered`/`lent` and while holding the lock (copy), so after a successful
//! cancel (or after `read-result`/`write-result`) the memory may be freed.
//! Dropping the last reader handle withdraws a pending read offer and write
//! offers, dropping the last writer handle withdraws a pending write offer
//! (not a detached one).
//!
//! # EOF
//!
//! Closed means no handle of that direction is left. Completed operations are
//! delivered before EOF is reported.

use std::{
    ptr,
    sync::{
        atomic::{AtomicU8, AtomicUsize, Ordering},
        Arc, Condvar, Mutex, MutexGuard,
    },
};

use stream_impl::exports::symmetric::runtime::symmetric_stream::{
    self, Address, GuestAddress, GuestBuffer, GuestStreamObj,
};
use stream_impl::symmetric::runtime::symmetric_executor::EventGenerator;

use crate::stream_impl::exports::symmetric::runtime::symmetric_stream::StreamState;

mod stream_impl;

struct Guest;

stream_impl::export!(Guest with_types_in stream_impl);

struct Dummy;

impl GuestAddress for Dummy {}

#[cfg(feature = "trace")]
fn gettid() -> libc::pid_t {
    unsafe { libc::gettid() }
}

#[cfg(feature = "trace")]
macro_rules! trace {
    ($($arg:tt)*) => { println!("{} {}", gettid(), format_args!($($arg)*)) };
}
#[cfg(not(feature = "trace"))]
macro_rules! trace {
    ($($arg:tt)*) => {};
}

struct Buffer {
    addr: *mut (),
    capacity: usize,
    element_size: usize,
    size: AtomicUsize,
}

impl GuestBuffer for Buffer {
    fn new(addr: symmetric_stream::Address, capacity: u64, element_size: u64) -> Self {
        Self {
            addr: addr.take_handle() as *mut (),
            size: AtomicUsize::new(0),
            capacity: capacity as usize,
            element_size: element_size as usize,
        }
    }

    fn get_address(&self) -> symmetric_stream::Address {
        unsafe { Address::from_handle(self.addr as usize) }
    }

    fn get_size(&self) -> u64 {
        self.size.load(Ordering::Relaxed) as u64
    }

    fn set_size(&self, size: u64) {
        self.size.store(size as usize, Ordering::Relaxed)
    }

    fn capacity(&self) -> u64 {
        self.capacity as u64
    }
}

/// The runtime's view of a buffer (an offered operation).
struct Offer {
    addr: usize,
    capacity: usize,
    element_size: usize,
    /// reader: unused until completed; writer: number of elements offered;
    /// completed: number of elements transferred
    size: usize,
    /// copy of the elements of a detached write (`addr` points into it)
    _owned: Option<Vec<u8>>,
}

impl Offer {
    fn from_buffer(buffer: &symmetric_stream::Buffer) -> Self {
        let b = buffer.get::<Buffer>();
        Offer {
            addr: b.addr as usize,
            capacity: b.capacity,
            element_size: b.element_size,
            size: b.size.load(Ordering::Relaxed),
            _owned: None,
        }
    }

    fn into_buffer(self) -> symmetric_stream::Buffer {
        symmetric_stream::Buffer::new(Buffer {
            addr: self.addr as *mut (),
            capacity: self.capacity,
            element_size: self.element_size,
            size: AtomicUsize::new(self.size),
        })
    }

    fn with_size(mut self, size: usize) -> Self {
        self.size = size;
        self
    }
}

#[derive(Default)]
struct State {
    /// posted by the reader, waiting for a writer
    read_offer: Option<Offer>,
    /// the offered buffer of the reader is borrowed by a writer
    /// (`start-writing` until `finish-writing`)
    read_lent: Option<Offer>,
    /// completed read, `size` is the count
    read_done: Option<Offer>,
    /// posted by the writer, waiting for a reader
    write_offer: Option<Offer>,
    /// the writer's offer is a detached one
    write_detached: bool,
    /// completed write, `size` is the count
    write_done: Option<Offer>,
    readers: usize,
    writers: usize,
}

struct StreamInner {
    state: Mutex<State>,
    /// signals "the borrowed buffer was returned"
    returned: Condvar,
    /// activated when something changed for the reader
    read_event: EventGenerator,
    /// activated when something changed for the writer
    write_event: EventGenerator,
}

impl StreamInner {
    fn handle(&self) -> usize {
        self.read_event.handle() as usize
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap()
    }

    /// wait until a writer has returned a borrowed read buffer
    fn wait_not_lent<'a>(&'a self, mut state: MutexGuard<'a, State>) -> MutexGuard<'a, State> {
        while state.read_lent.is_some() {
            state = self.returned.wait(state).unwrap();
        }
        state
    }

    /// A reader arrives with `read`, a writer offer may be waiting.
    fn post_read(&self, state: &mut State, mut read: Offer) {
        match state.write_offer.take() {
            None => {
                if state.writers == 0 {
                    // EOF, `read-result` reports it, nothing to offer
                    trace!("Stream {:x} start_reading eof", self.handle());
                } else {
                    trace!(
                        "Stream {:x} start_reading offered {}",
                        self.handle(),
                        read.capacity
                    );
                    state.read_offer = Some(read);
                }
                // writers waiting in start-writing learn about the offer, readers
                // are only woken for data (or EOF), clients rely on that
                self.write_event.activate();
                if state.writers == 0 {
                    self.read_event.activate();
                }
            }
            Some(mut write) => {
                let n = read.capacity.min(write.size);
                if n > 0 {
                    unsafe {
                        ptr::copy_nonoverlapping(
                            write.addr as *const u8,
                            read.addr as *mut u8,
                            n * write.element_size,
                        )
                    };
                    trace!("Stream {:x} start_reading copied {n}", self.handle());
                    read.size = n;
                    write.size = n;
                    state.read_done = Some(read);
                    if !std::mem::take(&mut state.write_detached) {
                        state.write_done = Some(write);
                    }
                } else if read.capacity == 0 {
                    // readiness only, the writer remains
                    read.size = 0;
                    state.read_done = Some(read);
                    state.write_offer = Some(write);
                } else {
                    // zero length write
                    write.size = 0;
                    if !std::mem::take(&mut state.write_detached) {
                        state.write_done = Some(write);
                    }
                    state.read_offer = Some(read);
                }
                self.read_event.activate();
                self.write_event.activate();
            }
        }
    }

    /// A writer arrives with `write` (not detached means: not copied)
    fn post_write(&self, state: &mut State, mut write: Offer, detached: bool) {
        if state.readers == 0 {
            // nobody to read it, `write-result` reports EOF
            self.write_event.activate();
            return;
        }
        match state.read_offer.take() {
            None => {
                trace!(
                    "Stream {:x} offer_write offered {}",
                    self.handle(),
                    write.size
                );
                state.write_offer = Some(write);
                state.write_detached = detached;
            }
            Some(mut read) => {
                let n = read.capacity.min(write.size);
                if n > 0 {
                    unsafe {
                        ptr::copy_nonoverlapping(
                            write.addr as *const u8,
                            read.addr as *mut u8,
                            n * write.element_size,
                        )
                    };
                    trace!("Stream {:x} offer_write copied {n}", self.handle());
                    read.size = n;
                    write.size = n;
                    state.read_done = Some(read);
                    if !detached {
                        state.write_done = Some(write);
                    }
                } else if read.capacity == 0 {
                    read.size = 0;
                    state.read_done = Some(read);
                    state.write_offer = Some(write);
                    state.write_detached = detached;
                } else {
                    write.size = 0;
                    if !detached {
                        state.write_done = Some(write);
                    }
                    state.read_offer = Some(read);
                }
                self.read_event.activate();
                self.write_event.activate();
            }
        }
    }

    /// the last reader is gone
    fn readers_closed<'a>(&'a self, mut state: MutexGuard<'a, State>) {
        state = self.wait_not_lent(state);
        state.read_offer = None;
        state.read_done = None;
        state.write_offer = None;
        state.write_detached = false;
        drop(state);
        trace!("Stream {:x} last reader closed", self.handle());
        self.write_event.activate();
    }

    /// the last writer is gone
    fn writers_closed<'a>(&'a self, mut state: MutexGuard<'a, State>) {
        // a detached write remains until consumed
        if !state.write_detached {
            state.write_offer = None;
        }
        drop(state);
        trace!("Stream {:x} last writer closed", self.handle());
        self.read_event.activate();
    }
}

#[repr(u8)]
enum DecreaseOnDrop {
    None,
    Reader,
    Writer,
}
type AtomicDecreaseOnDrop = AtomicU8;

struct StreamObj(Arc<StreamInner>, AtomicDecreaseOnDrop);

impl StreamObj {
    fn assert_direction(&self, direction: DecreaseOnDrop) {
        assert_eq!(self.1.load(Ordering::Relaxed), direction as u8);
    }

    fn close_reader(&self) {
        let mut state = self.0.lock();
        state.readers -= 1;
        if state.readers == 0 {
            self.0.readers_closed(state);
        }
    }
}

impl Drop for StreamObj {
    fn drop(&mut self) {
        match self.1.load(Ordering::Relaxed) {
            val if val == DecreaseOnDrop::None as u8 => (),
            val if val == DecreaseOnDrop::Reader as u8 => self.close_reader(),
            val if val == DecreaseOnDrop::Writer as u8 => {
                let mut state = self.0.lock();
                state.writers -= 1;
                if state.writers == 0 {
                    self.0.writers_closed(state);
                }
            }
            _ => unimplemented!("Invalid drop type"),
        }
    }
}

impl GuestStreamObj for StreamObj {
    fn new() -> Self {
        let inner = StreamInner {
            state: Mutex::new(State {
                writers: 1,
                ..Default::default()
            }),
            returned: Condvar::new(),
            read_event: EventGenerator::new(),
            write_event: EventGenerator::new(),
        };
        trace!("Stream::new {:x}", inner.handle());
        Self(
            Arc::new(inner),
            AtomicDecreaseOnDrop::new(DecreaseOnDrop::Writer as u8),
        )
    }

    fn clone(&self, writing: bool) -> symmetric_stream::StreamObj {
        {
            let mut state = self.0.lock();
            if writing {
                state.writers += 1;
            } else {
                state.readers += 1;
            }
            trace!(
                "Stream::clone({writing}) {:x} -> w {} r {}",
                self.0.handle(),
                state.writers,
                state.readers
            );
        }
        symmetric_stream::StreamObj::new(StreamObj(
            Arc::clone(&self.0),
            AtomicDecreaseOnDrop::new(if writing {
                DecreaseOnDrop::Writer
            } else {
                DecreaseOnDrop::Reader
            } as u8),
        ))
    }

    fn is_write_closed(&self) -> bool {
        self.assert_direction(DecreaseOnDrop::Reader);
        let state = self.0.lock();
        state.writers == 0 && state.write_offer.is_none()
    }

    fn start_reading(
        &self,
        buffer: symmetric_stream::Buffer,
    ) -> Result<(), symmetric_stream::Buffer> {
        self.assert_direction(DecreaseOnDrop::Reader);
        let read = Offer::from_buffer(&buffer);
        let mut state = self.0.lock();
        if state.read_offer.is_some() || state.read_lent.is_some() || state.read_done.is_some() {
            trace!("Stream::start_reading {:x} failed", self.0.handle());
            return Err(buffer);
        }
        self.0.post_read(&mut state, read);
        Ok(())
    }

    fn read_ready_subscribe(&self) -> symmetric_stream::EventSubscription {
        self.assert_direction(DecreaseOnDrop::Reader);
        self.0.read_event.subscribe()
    }

    fn read_result(&self) -> Result<symmetric_stream::Buffer, StreamState> {
        self.assert_direction(DecreaseOnDrop::Reader);
        let mut state = self.0.lock();
        if let Some(done) = state.read_done.take() {
            trace!(
                "Stream::read_result {:x} {:x} {}",
                self.0.handle(),
                done.addr,
                done.size
            );
            Ok(done.into_buffer())
        } else if state.writers == 0 && state.write_offer.is_none() {
            // the offer (if any) is withdrawn, the reader may free its memory
            state = self.0.wait_not_lent(state);
            state.read_offer = None;
            trace!("Stream::read_result {:x} eof", self.0.handle());
            Err(StreamState::Eof)
        } else {
            trace!("Stream::read_result {:x} pending", self.0.handle());
            Err(StreamState::Pending)
        }
    }

    fn cancel_read(&self) -> Option<symmetric_stream::Buffer> {
        self.assert_direction(DecreaseOnDrop::Reader);
        let mut state = self.0.lock();
        state = self.0.wait_not_lent(state);
        let res = if let Some(done) = state.read_done.take() {
            Some(done)
        } else {
            state.read_offer.take().map(|offer| offer.with_size(0))
        };
        trace!(
            "Stream::cancel_read {:x} -> {:?}",
            self.0.handle(),
            res.as_ref().map(|o| o.size)
        );
        res.map(Offer::into_buffer)
    }

    fn close_read(&self) -> Option<symmetric_stream::Buffer> {
        self.assert_direction(DecreaseOnDrop::Reader);
        let res = self.cancel_read();
        // closing the read side is what dropping the reader does, signal it
        // to the writers (and don't do it again on drop)
        self.1.store(DecreaseOnDrop::None as u8, Ordering::Release);
        self.close_reader();
        res
    }

    fn is_read_closed(&self) -> bool {
        self.0.lock().readers == 0
    }

    fn start_writing(&self) -> Result<symmetric_stream::Buffer, StreamState> {
        self.assert_direction(DecreaseOnDrop::Writer);
        let mut state = self.0.lock();
        if let Some(offer) = state.read_offer.take() {
            trace!(
                "Stream::start_writing {:x} {:x} {}",
                self.0.handle(),
                offer.addr,
                offer.capacity
            );
            let buffer = Offer {
                addr: offer.addr,
                capacity: offer.capacity,
                element_size: offer.element_size,
                size: 0,
                _owned: None,
            }
            .into_buffer();
            state.read_lent = Some(offer);
            Ok(buffer)
        } else if state.readers == 0 {
            trace!("Stream::start_writing {:x} eof", self.0.handle());
            Err(StreamState::Eof)
        } else {
            trace!("Stream::start_writing {:x} pending", self.0.handle());
            Err(StreamState::Pending)
        }
    }

    fn write_ready_subscribe(&self) -> symmetric_stream::EventSubscription {
        self.assert_direction(DecreaseOnDrop::Writer);
        self.0.write_event.subscribe()
    }

    fn finish_writing(
        &self,
        buffer: symmetric_stream::Buffer,
    ) -> Result<(), symmetric_stream::Buffer> {
        self.assert_direction(DecreaseOnDrop::Writer);
        let count = buffer.get::<Buffer>().get_size() as usize;
        let mut state = self.0.lock();
        match state.read_lent.take() {
            Some(mut read) => {
                assert!(count <= read.capacity);
                trace!("Stream::finish_writing {:x} {count}", self.0.handle());
                read.size = count;
                state.read_done = Some(read);
                drop(state);
                self.0.returned.notify_all();
                self.0.read_event.activate();
                Ok(())
            }
            None => {
                trace!("Stream::finish_writing {:x} failed", self.0.handle());
                Err(buffer)
            }
        }
    }

    fn offer_write(
        &self,
        buffer: symmetric_stream::Buffer,
    ) -> Result<(), symmetric_stream::Buffer> {
        self.assert_direction(DecreaseOnDrop::Writer);
        let write = Offer::from_buffer(&buffer);
        assert!(write.size <= write.capacity);
        let mut state = self.0.lock();
        if state.write_offer.is_some() || state.write_done.is_some() {
            trace!("Stream::offer_write {:x} failed", self.0.handle());
            return Err(buffer);
        }
        self.0.post_write(&mut state, write, false);
        Ok(())
    }

    fn write_detached(&self, buffer: symmetric_stream::Buffer) {
        self.assert_direction(DecreaseOnDrop::Writer);
        let mut write = Offer::from_buffer(&buffer);
        assert!(write.size <= write.capacity);
        let mut copy = vec![0u8; write.size * write.element_size];
        unsafe { ptr::copy_nonoverlapping(write.addr as *const u8, copy.as_mut_ptr(), copy.len()) };
        write.addr = copy.as_ptr() as usize;
        write._owned = Some(copy);
        let mut state = self.0.lock();
        if state.write_offer.is_some() {
            // a write is in flight, there is nothing sensible to do with a
            // second one, the elements are dropped
            return;
        }
        self.0.post_write(&mut state, write, true);
    }

    fn cancel_write(&self) -> Option<symmetric_stream::Buffer> {
        self.assert_direction(DecreaseOnDrop::Writer);
        let mut state = self.0.lock();
        let res = if let Some(done) = state.write_done.take() {
            Some(done)
        } else if state.write_detached {
            None
        } else {
            state.write_offer.take().map(|offer| offer.with_size(0))
        };
        trace!(
            "Stream::cancel_write {:x} -> {:?}",
            self.0.handle(),
            res.as_ref().map(|o| o.size)
        );
        res.map(Offer::into_buffer)
    }

    fn write_result(&self) -> Result<symmetric_stream::Buffer, StreamState> {
        self.assert_direction(DecreaseOnDrop::Writer);
        let mut state = self.0.lock();
        if let Some(done) = state.write_done.take() {
            trace!(
                "Stream::write_result {:x} {:x} {}",
                self.0.handle(),
                done.addr,
                done.size
            );
            Ok(done.into_buffer())
        } else if state.readers == 0 {
            if !state.write_detached {
                state.write_offer = None;
            }
            trace!("Stream::write_result {:x} eof", self.0.handle());
            Err(StreamState::Eof)
        } else {
            trace!("Stream::write_result {:x} pending", self.0.handle());
            Err(StreamState::Pending)
        }
    }
}

impl symmetric_stream::Guest for Guest {
    type Address = Dummy;

    type Buffer = Buffer;

    type StreamObj = StreamObj;
}
