use std::{
    cell::UnsafeCell,
    num::NonZero,
    ptr::null_mut,
    sync::{
        atomic::{AtomicBool, AtomicIsize, AtomicPtr, AtomicU8, AtomicUsize, Ordering},
        Arc,
    },
};

use stream_impl::exports::symmetric::runtime::symmetric_stream::{
    self, Address, GuestAddress, GuestBuffer, GuestStreamObj,
};
use stream_impl::symmetric::runtime::symmetric_executor::EventGenerator;

mod stream_impl;

struct Guest;

stream_impl::export!(Guest with_types_in stream_impl);

struct Dummy;

impl GuestAddress for Dummy {}

struct Buffer {
    addr: *mut (),
    capacity: usize,
    size: AtomicUsize,
}

impl GuestBuffer for Buffer {
    fn new(addr: symmetric_stream::Address, capacity: u64) -> Self {
        Self {
            addr: addr.take_handle() as *mut (),
            size: AtomicUsize::new(0),
            capacity: capacity as usize,
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

// mod results {
//     pub const BLOCKED: isize = -1;
// }

// TODO: (all internal)
// - replace close with number of writers/readers
// - separate variables into two generic single direction channels
// - remove BLOCKED

type ChannelSync = AtomicUsize;
const MULTI_HEAD: bool = false;
const BUSY: usize = usize::MAX;

struct SingleDirectionChannel<PAYLOAD> {
    ready_event: EventGenerator,
    size: ChannelSync,
    payload: UnsafeCell<PAYLOAD>,
    number_of_writers: AtomicUsize,
}

impl<P: Default> Default for SingleDirectionChannel<P> {
    fn default() -> Self {
        Self {
            ready_event: EventGenerator::new(),
            size: Default::default(),
            payload: Default::default(),
            number_of_writers: Default::default(),
        }
    }
}

impl<P> SingleDirectionChannel<P> {
    // this assumes that you already acquired or reset the other-directional signal
    fn write(&self, size: NonZero<usize>, payload: P) -> Result<(), (NonZero<usize>, P)> {
        let busy = if MULTI_HEAD {
            self.size
                .compare_exchange(0, BUSY, Ordering::Acquire, Ordering::Relaxed)
                .is_err()
        } else {
            self.size.load(Ordering::Acquire) != 0
        };
        if busy {
            Err((size, payload))
        } else {
            // Safety: The payload is protected by size
            unsafe { *self.payload.get() = payload };
            self.size.store(size.get(), Ordering::Release);
            self.ready_event.activate();
            Ok(())
        }
    }

    // this assumes that you already acquired or reset the other-directional signal
    fn read(&self) -> Result<(NonZero<usize>, P), ()> {
        let size = self.size.load(Ordering::Acquire);
        if size == 0 || (MULTI_HEAD && size == BUSY) {
            Err(())
        } else {
            if MULTI_HEAD
                && self
                    .size
                    .compare_exchange(size, BUSY, Ordering::Acquire, Ordering::Relaxed)
                    != Ok(size)
            {
                Err(())
            } else {
                // Safety: The payload is protected by size
                let payload = unsafe { self.payload.get().read() };
                self.size.store(0, Ordering::Release);
                let nonzero_size = unsafe { NonZero::new_unchecked(size) };
                Ok((nonzero_size, payload))
            }
        }
    }

    fn add_writer(&self) {
        self.number_of_writers.fetch_add(1, Ordering::Relaxed);
    }

    fn drop_writer(&self) {
        let prev = self.number_of_writers.fetch_sub(1, Ordering::Relaxed);
        assert!(prev != 0);
    }

    fn has_writers(&self) -> bool {
        self.number_of_writers.load(Ordering::Acquire) > 0
    }
}

struct StreamInner {
    // reader to writer (address)
    empty_buffer: SingleDirectionChannel<*mut ()>,
    // writer to reader (address+capacity)
    full_buffer: SingleDirectionChannel<(*mut (), usize)>,
    // read_ready_event_send: EventGenerator,
    // write_ready_event_send: EventGenerator,
    // read_addr: AtomicPtr<()>,
    // read_size: AtomicUsize,
    // ready_addr: AtomicPtr<()>,
    // ready_size: AtomicIsize,
    // ready_capacity: AtomicUsize,
    // // if the writer closes before the reader has consumed the last data
    // write_closed: AtomicBool,
    // // reader closed
    // read_closed: AtomicBool,
}

#[repr(u8)]
enum DecreaseOnDrop {
    None,
    Reader,
    Writer,
}

type AtomicDecreaseOnDrop = AtomicU8;

// bool = is_writer
struct StreamObj(Arc<StreamInner>, AtomicDecreaseOnDrop);

impl Drop for StreamObj {
    fn drop(&mut self) {
        match self.1.load(Ordering::Relaxed) {
            val if val == DecreaseOnDrop::None as u8 => (),
            val if val == DecreaseOnDrop::Reader as u8 => self.0.empty_buffer.drop_writer(),
            val if val == DecreaseOnDrop::Writer as u8 => self.0.full_buffer.drop_writer(),
            _ => unimplemented!("Invalid drop type"),
        }
    }
}

impl GuestStreamObj for StreamObj {
    fn new() -> Self {
        let inner = StreamInner {
            empty_buffer: Default::default(),
            full_buffer: Default::default(),
        };
        inner.full_buffer.add_writer();
        #[cfg(feature = "trace")]
        println!("Stream::new {:x}", inner.read_ready_event_send.handle());
        Self(
            Arc::new(inner),
            AtomicDecreaseOnDrop::new(DecreaseOnDrop::Writer as u8),
        )
    }

    fn is_write_closed(&self) -> bool {
        !self.0.full_buffer.has_writers()
        // self.0.ready_addr.load(Ordering::Acquire) as usize == EOF_MARKER
        //     || self.0.write_closed.load(Ordering::Acquire)
    }

    // pass buffer to reading side
    fn start_reading(&self, buffer: symmetric_stream::Buffer) {
        let buf = buffer.get::<Buffer>().get_address().take_handle() as *mut ();
        let size = buffer.get::<Buffer>().capacity();
        #[cfg(feature = "trace")]
        println!(
            "Stream::start_read {:x} {buf:x?} {size} =>",
            self.0.read_ready_event_send.handle()
        );
        let res = self.0.empty_buffer.write(size, buf);

        let old_readya = self.0.ready_addr.load(Ordering::Acquire);
        let old_ready = self.0.ready_size.load(Ordering::Acquire);
        if old_readya as usize == EOF_MARKER {
            todo!();
        }
        assert!(old_ready == results::BLOCKED);
        let old_size = self.0.read_size.swap(size as usize, Ordering::Acquire);
        assert_eq!(old_size, 0);
        let old_ptr = self.0.read_addr.swap(buf, Ordering::Release);
        assert_eq!(old_ptr, std::ptr::null_mut());
        self.write_ready_activate();
    }

    fn read_result(&self) -> Option<symmetric_stream::Buffer> {
        let size = self.0.ready_size.swap(results::BLOCKED, Ordering::Acquire);
        let addr = self.0.ready_addr.swap(null_mut(), Ordering::Relaxed);
        let capacity = self.0.ready_capacity.swap(0, Ordering::Relaxed);
        #[cfg(feature = "trace")]
        println!(
            "Stream::read_result {:x} {addr:x?} {size}",
            self.0.read_ready_event_send.handle()
        );
        if addr as usize == EOF_MARKER || (addr.is_null() && size == results::BLOCKED) {
            None
        } else {
            Some(symmetric_stream::Buffer::new(Buffer {
                addr,
                capacity,
                size: AtomicUsize::new(size as usize),
            }))
        }
    }

    // fn is_ready_to_write(&self) -> bool {
    //     !self.0.read_addr.load(Ordering::Acquire).is_null()
    // }

    fn start_writing(&self) -> Result<symmetric_stream::Buffer, symmetric_stream::StreamState> {
        let size = self.0.read_size.swap(0, Ordering::Acquire);
        let addr = self
            .0
            .read_addr
            .swap(core::ptr::null_mut(), Ordering::Relaxed);
        #[cfg(feature = "trace")]
        println!(
            "Stream::start_write {:x} {addr:x?} {size}",
            self.0.read_ready_event_send.handle()
        );
        self.0.ready_capacity.store(size, Ordering::Release);
        symmetric_stream::Buffer::new(Buffer {
            addr,
            capacity: size,
            size: AtomicUsize::new(0),
        })
    }

    fn finish_writing(&self, buffer: Option<symmetric_stream::Buffer>) {
        let (elements, addr) = if let Some(buffer) = buffer {
            let elements = buffer.get::<Buffer>().get_size() as isize;
            let addr = buffer.get::<Buffer>().get_address().take_handle() as *mut ();
            (elements, addr)
        } else {
            if self.is_write_closed() {
                todo!("double close");
            }
            if !self.0.ready_addr.load(Ordering::Relaxed).is_null() {
                self.0.write_closed.store(true, Ordering::Release);
                #[cfg(feature = "trace")]
                println!(
                    "Stream::finish_write CLOSE {:x}",
                    self.0.read_ready_event_send.handle()
                );
                return;
            }
            (0, EOF_MARKER as *mut ())
        };
        #[cfg(feature = "trace")]
        println!(
            "Stream::finish_write {:x} {addr:x?} {elements} =>",
            self.0.read_ready_event_send.handle()
        );
        let old_ready = self.0.ready_size.swap(elements, Ordering::Relaxed);
        let _old_ready_addr = self.0.ready_addr.swap(addr, Ordering::Release);
        assert_eq!(old_ready, results::BLOCKED);
        self.read_ready_activate();
    }

    fn clone(&self) -> symmetric_stream::StreamObj {
        self.0.empty_buffer.add_writer();
        symmetric_stream::StreamObj::new(StreamObj(
            Arc::clone(&self.0),
            AtomicDecreaseOnDrop::new(DecreaseOnDrop::Reader as u8),
        ))
    }

    fn write_ready_activate(&self) {
        self.0.write_ready_event_send.activate();
    }

    fn read_ready_subscribe(&self) -> symmetric_stream::EventSubscription {
        self.0.read_ready_event_send.subscribe()
    }

    fn write_ready_subscribe(&self) -> symmetric_stream::EventSubscription {
        self.0.write_ready_event_send.subscribe()
    }

    fn read_ready_activate(&self) {
        self.0.read_ready_event_send.activate();
    }

    // TODO: Also add unread full buffers?
    fn close_read(&self) -> Vec<symmetric_stream::Buffer> {
        let mut res = Vec::new();
        let size = self.0.read_size.swap(0, Ordering::Acquire);
        let addr = self
            .0
            .read_addr
            .swap(core::ptr::null_mut(), Ordering::Relaxed);
        #[cfg(feature = "trace")]
        println!("Stream::close_read {addr:x?} {size}",);
        self.0.read_closed.store(true, Ordering::Release);
        self.write_ready_activate();
        if size > 0 {
            let buffer = symmetric_stream::Buffer::new(Buffer {
                addr,
                capacity: size,
                size: AtomicUsize::new(0),
            });
            res.push(buffer);
        }
        assert!(self.1.load(Ordering::Acquire) == DecreaseOnDrop::Reader as u8);
        self.1.store(DecreaseOnDrop::None as u8, Ordering::Release);
        res
    }

    fn is_read_closed(&self) -> bool {
        self.0.read_closed.load(Ordering::Relaxed)
    }
}

const EOF_MARKER: usize = 1;

impl symmetric_stream::Guest for Guest {
    type Address = Dummy;

    type Buffer = Buffer;

    type StreamObj = StreamObj;
}
