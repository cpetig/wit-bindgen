use std::{
    cell::UnsafeCell,
    num::NonZero,
    sync::{
        atomic::{AtomicU8, AtomicUsize, Ordering},
        Arc,
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

    // true if this was the last one
    fn drop_writer(&self) -> bool {
        let prev = self.number_of_writers.fetch_sub(1, Ordering::Relaxed);
        if prev == 1 {
            // signal EOF to other side
            self.ready_event.activate();
        }
        assert!(prev != 0);
        prev == 1
    }

    fn has_writers(&self) -> bool {
        self.number_of_writers.load(Ordering::Acquire) > 0
    }

    fn subscribe(&self) -> symmetric_stream::EventSubscription {
        self.ready_event.subscribe()
    }

    fn handle(&self) -> usize {
        self.ready_event.handle() as usize
    }
}

struct StreamInner {
    // reader to writer (address)
    empty_buffer: SingleDirectionChannel<*mut ()>,
    // writer to reader (address+capacity)
    full_buffer: SingleDirectionChannel<(*mut (), usize)>,
}

impl StreamInner {
    fn handle(&self) -> usize {
        self.empty_buffer.handle()
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

impl Drop for StreamObj {
    fn drop(&mut self) {
        match self.1.load(Ordering::Relaxed) {
            val if val == DecreaseOnDrop::None as u8 => (),
            val if val == DecreaseOnDrop::Reader as u8 => {
                if self.0.empty_buffer.drop_writer() {
                    #[cfg(feature = "trace")]
                    println!("Stream last reader dropped {:x}", self.0.handle());
                }
            }
            val if val == DecreaseOnDrop::Writer as u8 => {
                if self.0.full_buffer.drop_writer() {
                    #[cfg(feature = "trace")]
                    println!("Stream last writer dropped {:x}", self.0.handle());
                }
            }
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
        println!("Stream::new {:x}", inner.handle());
        Self(
            Arc::new(inner),
            AtomicDecreaseOnDrop::new(DecreaseOnDrop::Writer as u8),
        )
    }

    fn is_write_closed(&self) -> bool {
        assert_eq!(self.1.load(Ordering::Relaxed), DecreaseOnDrop::Reader as u8);
        !self.0.full_buffer.has_writers()
    }

    // pass buffer to reading side
    fn start_reading(
        &self,
        buffer: symmetric_stream::Buffer,
    ) -> Result<(), symmetric_stream::Buffer> {
        assert_eq!(self.1.load(Ordering::Relaxed), DecreaseOnDrop::Reader as u8);
        let buf = buffer.get::<Buffer>().get_address().take_handle() as *mut ();
        let size = buffer.get::<Buffer>().capacity();
        #[cfg(feature = "trace")]
        println!(
            "Stream::start_read {:x} {buf:x?} {size} =>",
            self.0.handle()
        );
        let res = self
            .0
            .empty_buffer
            .write(NonZero::new(size as usize).unwrap(), buf);
        res.map_err(|(capacity, addr)| {
            symmetric_stream::Buffer::new(Buffer {
                addr,
                capacity: capacity.get(),
                size: AtomicUsize::new(0),
            })
        })
    }

    fn read_result(&self) -> Result<symmetric_stream::Buffer, StreamState> {
        assert_eq!(self.1.load(Ordering::Relaxed), DecreaseOnDrop::Reader as u8);
        let res = self.0.full_buffer.read();
        match res {
            Ok((size, (addr, capacity))) => {
                #[cfg(feature = "trace")]
                println!("Stream::read_result {:x} {addr:x?} {size}", self.0.handle());
                Ok(symmetric_stream::Buffer::new(Buffer {
                    addr,
                    capacity,
                    size: AtomicUsize::new(size.get()),
                }))
            }
            Err(()) => Err(if self.0.full_buffer.has_writers() {
                StreamState::Pending
            } else {
                StreamState::Eof
            }),
        }
    }

    fn start_writing(&self) -> Result<symmetric_stream::Buffer, StreamState> {
        assert_eq!(self.1.load(Ordering::Relaxed), DecreaseOnDrop::Writer as u8);
        let res = self.0.empty_buffer.read();
        match res {
            Ok((size, addr)) => {
                #[cfg(feature = "trace")]
                println!("Stream::start_write {:x} {addr:x?} {size}", self.0.handle());
                Ok(symmetric_stream::Buffer::new(Buffer {
                    addr,
                    capacity: size.get(),
                    size: AtomicUsize::new(0),
                }))
            }
            Err(()) => Err(if self.0.empty_buffer.has_writers() {
                StreamState::Pending
            } else {
                StreamState::Eof
            }),
        }
    }

    fn finish_writing(
        &self,
        buffer: symmetric_stream::Buffer,
    ) -> Result<(), symmetric_stream::Buffer> {
        assert_eq!(self.1.load(Ordering::Relaxed), DecreaseOnDrop::Writer as u8);
        let elements = buffer.get::<Buffer>().get_size() as usize;
        let addr = buffer.get::<Buffer>().get_address().take_handle() as *mut ();
        let capacity = buffer.get::<Buffer>().capacity() as usize;
        let res = self
            .0
            .full_buffer
            .write(NonZero::new(elements).unwrap(), (addr, capacity));
        if res.is_ok() {
            #[cfg(feature = "trace")]
            println!(
                "Stream::finish_write {:x} {addr:x?} {elements} =>",
                self.0.handle()
            );
        }
        res.map_err(|(size, (addr, capacity))| {
            symmetric_stream::Buffer::new(Buffer {
                addr,
                capacity,
                size: AtomicUsize::new(size.get()),
            })
        })
    }

    fn clone(&self, writing: bool) -> symmetric_stream::StreamObj {
        if !writing {
            self.0.empty_buffer.add_writer();
        } else {
            self.0.full_buffer.add_writer();
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

    fn read_ready_subscribe(&self) -> symmetric_stream::EventSubscription {
        assert_eq!(self.1.load(Ordering::Relaxed), DecreaseOnDrop::Reader as u8);
        self.0.full_buffer.subscribe()
    }

    fn write_ready_subscribe(&self) -> symmetric_stream::EventSubscription {
        assert_eq!(self.1.load(Ordering::Relaxed), DecreaseOnDrop::Writer as u8);
        self.0.empty_buffer.subscribe()
    }

    fn close_read(&self) -> Vec<symmetric_stream::Buffer> {
        assert_eq!(self.1.load(Ordering::Relaxed), DecreaseOnDrop::Reader as u8);
        let mut res = Vec::new();
        let pending_write = self.0.empty_buffer.read();
        match pending_write {
            Ok((size, addr)) => {
                let buffer = symmetric_stream::Buffer::new(Buffer {
                    addr,
                    capacity: size.get(),
                    size: AtomicUsize::new(0),
                });
                #[cfg(feature = "trace")]
                println!("Stream::close_read {addr:x?} {size}",);
                res.push(buffer);
            }
            Err(()) => (),
        }
        let pending_read = self.0.full_buffer.read();
        match pending_read {
            Ok((size, (addr, capacity))) => {
                let buffer = symmetric_stream::Buffer::new(Buffer {
                    addr,
                    capacity,
                    size: AtomicUsize::new(size.get()),
                });
                #[cfg(feature = "trace")]
                println!("Stream::close_read {addr:x?} {size}",);
                res.push(buffer);
            }
            Err(()) => (),
        }
        self.1.store(DecreaseOnDrop::None as u8, Ordering::Release);
        res
    }

    fn is_read_closed(&self) -> bool {
        !self.0.empty_buffer.has_writers()
    }
}

impl symmetric_stream::Guest for Guest {
    type Address = Dummy;

    type Buffer = Buffer;

    type StreamObj = StreamObj;
}
