use std::{
    alloc::Layout,
    future::{Future, IntoFuture},
    mem,
    pin::Pin,
    task::{Context, Poll},
};

use crate::symmetric_stream::{Address, Buffer, StreamState};

use super::{super::Cleanup, poll_event, Stream};

#[doc(hidden)]
pub struct FutureVtable<T> {
    pub layout: Layout,
    pub lower: unsafe fn(value: T, dst: *mut u8),
    pub lift: unsafe fn(dst: *mut u8) -> T,
}

/// Memory holding the lowered value of an in-flight operation, it has to
/// stay valid until the operation completed or was cancelled.
struct Lowered {
    ptr: *mut u8,
    _cleanup: Option<Cleanup>,
}

// the pointer is only used by the owner (and the stream while offered)
unsafe impl Send for Lowered {}

impl Lowered {
    fn new(layout: Layout) -> Self {
        let (ptr, cleanup) = Cleanup::new(layout);
        Self {
            ptr,
            _cleanup: cleanup,
        }
    }

    fn buffer(&self, layout: Layout, size: u64) -> Buffer {
        let address = unsafe { Address::from_handle(self.ptr as usize) };
        let buffer = Buffer::new(address, 1, layout.size() as u64);
        buffer.set_size(size);
        buffer
    }
}

/// The writing end of a future, the value of the future is provided by
/// [`FutureWriter::write`]. Dropping it without writing resolves the
/// future with the default value (unless the reader is gone).
pub struct FutureWriter<T: 'static> {
    handle: Stream,
    vtable: &'static FutureVtable<T>,
    default: Option<fn() -> T>,
    // a value was written (or there is nobody to write it to)
    completed: bool,
}

impl<T> FutureWriter<T> {
    pub fn new(handle: Stream, vtable: &'static FutureVtable<T>) -> Self {
        Self {
            handle,
            vtable,
            default: None,
            completed: false,
        }
    }

    pub fn write(self, data: T) -> FutureWrite<T> {
        FutureWrite {
            writer: Some(self),
            state: WriteState::Init(data),
        }
    }
}

impl<T> Drop for FutureWriter<T> {
    fn drop(&mut self) {
        // the reader would never see a value otherwise, the copy is taken
        // right away so nothing needs to stay alive
        if !self.completed && self.handle.handle() != 0 && !self.handle.is_read_closed() {
            if let Some(default) = self.default {
                let lowered = Lowered::new(self.vtable.layout);
                unsafe { (self.vtable.lower)(default(), lowered.ptr) };
                self.handle
                    .write_detached(lowered.buffer(self.vtable.layout, 1));
            }
        }
    }
}

enum WriteState<T> {
    // not yet started, holds the value
    Init(T),
    // the lowered value is offered to the reader
    Offered(Lowered),
    Done,
}

/// Represents a write operation which may be canceled prior to completion.
pub struct FutureWrite<T: 'static> {
    writer: Option<FutureWriter<T>>,
    state: WriteState<T>,
}

#[derive(Debug)]
pub struct FutureWriteError<T> {
    pub value: T,
}

impl<T: Unpin + Send> Future for FutureWrite<T> {
    type Output = Result<(), FutureWriteError<T>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let me = self.get_mut();
        let writer = me.writer.as_mut().expect("polled after completion");
        let vtable = writer.vtable;
        loop {
            match &me.state {
                WriteState::Init(_) => {
                    let WriteState::Init(value) = mem::replace(&mut me.state, WriteState::Done)
                    else {
                        unreachable!()
                    };
                    match writer.handle.start_writing() {
                        // the reader is waiting, write directly into its buffer
                        Ok(buffer) => {
                            let addr = buffer.get_address().take_handle() as *mut u8;
                            unsafe { (vtable.lower)(value, addr) };
                            buffer.set_size(1);
                            writer
                                .handle
                                .finish_writing(buffer)
                                .unwrap_or_else(|_| unreachable!("borrowed buffer rejected"));
                            writer.completed = true;
                            return Poll::Ready(Ok(()));
                        }
                        Err(StreamState::Eof) => {
                            writer.completed = true;
                            return Poll::Ready(Err(FutureWriteError { value }));
                        }
                        // the writer is first, offer a buffer
                        Err(StreamState::Pending) => {
                            let lowered = Lowered::new(vtable.layout);
                            unsafe { (vtable.lower)(value, lowered.ptr) };
                            writer
                                .handle
                                .offer_write(lowered.buffer(vtable.layout, 1))
                                .unwrap_or_else(|_| unreachable!("write already in flight"));
                            me.state = WriteState::Offered(lowered);
                        }
                    }
                }
                WriteState::Offered(lowered) => {
                    let subscription = writer.handle.write_ready_subscribe();
                    subscription.reset();
                    match writer.handle.write_result() {
                        Ok(_) => {
                            // the single element was taken
                            me.state = WriteState::Done;
                            writer.completed = true;
                            return Poll::Ready(Ok(()));
                        }
                        Err(StreamState::Eof) => {
                            let value = unsafe { (vtable.lift)(lowered.ptr) };
                            me.state = WriteState::Done;
                            writer.completed = true;
                            return Poll::Ready(Err(FutureWriteError { value }));
                        }
                        Err(StreamState::Pending) => {
                            if poll_event(cx, &subscription).is_pending() {
                                return Poll::Pending;
                            }
                        }
                    }
                }
                WriteState::Done => panic!("polled after completion"),
            }
        }
    }
}

impl<T> Drop for FutureWrite<T> {
    fn drop(&mut self) {
        // take the offer back before the memory goes away
        if let (WriteState::Offered(lowered), Some(writer)) = (&self.state, &mut self.writer) {
            match writer.handle.cancel_write() {
                Some(buffer) if buffer.get_size() >= 1 => writer.completed = true,
                _ => drop(unsafe { (writer.vtable.lift)(lowered.ptr) }),
            }
        }
    }
}

impl<T> std::fmt::Debug for FutureWriter<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FutureWriter")
            .field("handle", &self.handle.handle())
            .finish()
    }
}

impl<T: 'static + std::marker::Unpin> FutureWrite<T> {
    pub fn cancel(self: Pin<&mut Self>) -> FutureWriteCancel<T> {
        let me = self.get_mut();
        let Some(mut writer) = me.writer.take() else {
            return FutureWriteCancel::AlreadySent;
        };
        match mem::replace(&mut me.state, WriteState::Done) {
            WriteState::Init(value) => {
                if writer.handle.is_read_closed() {
                    writer.completed = true;
                    FutureWriteCancel::Dropped(value)
                } else {
                    FutureWriteCancel::Cancelled(value, writer)
                }
            }
            WriteState::Offered(lowered) => match writer.handle.cancel_write() {
                // the reader took the value (the cancel raced the completion)
                Some(buffer) if buffer.get_size() >= 1 => {
                    writer.completed = true;
                    FutureWriteCancel::AlreadySent
                }
                _ => {
                    let value = unsafe { (writer.vtable.lift)(lowered.ptr) };
                    if writer.handle.is_read_closed() {
                        writer.completed = true;
                        FutureWriteCancel::Dropped(value)
                    } else {
                        FutureWriteCancel::Cancelled(value, writer)
                    }
                }
            },
            WriteState::Done => FutureWriteCancel::AlreadySent,
        }
    }
}

#[derive(Debug)]
pub enum FutureWriteCancel<T: 'static> {
    Cancelled(T, FutureWriter<T>),
    Dropped(T),
    AlreadySent,
}

/// Represents a read operation which may be canceled prior to completion.
pub struct FutureRead<T: 'static> {
    // None once the read has completed
    reader: Option<FutureReader<T>>,
    // the buffer offered to the stream while a read is in flight
    op: Option<Lowered>,
}

pub struct FutureReader<T: 'static> {
    handle: Stream,
    vtable: &'static FutureVtable<T>,
}

impl<T> FutureReader<T> {
    pub fn new(handle: *mut u8, vtable: &'static FutureVtable<T>) -> Self {
        Self {
            handle: unsafe { Stream::from_handle(handle as usize) },
            vtable,
        }
    }

    pub fn read(self) -> FutureRead<T> {
        FutureRead {
            reader: Some(self),
            op: None,
        }
    }

    pub unsafe fn from_handle(handle: *mut u8, vtable: &'static FutureVtable<T>) -> Self {
        Self::new(handle, vtable)
    }

    pub fn take_handle(&self) -> *mut () {
        self.handle.take_handle() as *mut ()
    }
}

impl<T> std::fmt::Debug for FutureReader<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FutureReader")
            .field("handle", &self.handle.handle())
            .finish()
    }
}

impl<T> FutureRead<T> {
    // offer a buffer for the value (the stream completes it as soon as a writer is there)
    fn start(reader: &FutureReader<T>) -> Lowered {
        let lowered = Lowered::new(reader.vtable.layout);
        reader
            .handle
            .start_reading(lowered.buffer(reader.vtable.layout, 0))
            .unwrap_or_else(|_| unreachable!("read already in flight"));
        lowered
    }
}

impl<T: Unpin + Sized + Send> Future for FutureRead<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<T> {
        let me = self.get_mut();
        let reader = me.reader.as_ref().expect("polled after completion");
        if me.op.is_none() {
            me.op = Some(Self::start(reader));
        }
        loop {
            let lowered = me.op.as_ref().unwrap();
            let subscription = reader.handle.read_ready_subscribe();
            subscription.reset();
            match reader.handle.read_result() {
                Ok(buffer) => {
                    if buffer.get_size() > 0 {
                        let value = unsafe { (reader.vtable.lift)(lowered.ptr) };
                        me.op = None;
                        // the read consumes the reader
                        me.reader = None;
                        return Poll::Ready(value);
                    }
                    // completed empty, wait for the value
                    me.op = Some(Self::start(reader));
                }
                Err(StreamState::Eof) => {
                    me.op = None;
                    panic!("future writer dropped without a value")
                }
                Err(StreamState::Pending) => {
                    if poll_event(cx, &subscription).is_pending() {
                        return Poll::Pending;
                    }
                }
            }
        }
    }
}

impl<T> Drop for FutureRead<T> {
    fn drop(&mut self) {
        // take the offered buffer back before it is freed
        if let (Some(lowered), Some(reader)) = (&self.op, &self.reader) {
            if let Some(buffer) = reader.handle.cancel_read() {
                if buffer.get_size() > 0 {
                    drop(unsafe { (reader.vtable.lift)(lowered.ptr) });
                }
            }
        }
    }
}

impl<T> FutureRead<T> {
    /// Cancel the read. Returns the value if it was already written,
    /// otherwise the reader to try again later.
    pub fn cancel(self: Pin<&mut Self>) -> Result<T, FutureReader<T>> {
        let me = self.get_mut();
        let reader = me.reader.take().expect("read already completed");
        if let Some(lowered) = me.op.take() {
            if let Some(buffer) = reader.handle.cancel_read() {
                if buffer.get_size() > 0 {
                    return Ok(unsafe { (reader.vtable.lift)(lowered.ptr) });
                }
            }
        }
        Err(reader)
    }
}

impl<T: Send + Unpin + Sized> IntoFuture for FutureReader<T> {
    type Output = T;
    type IntoFuture = FutureRead<T>;

    /// Convert this object into a `Future` which will resolve when a value is
    /// written to the writable end of this `future`.
    fn into_future(self) -> Self::IntoFuture {
        self.read()
    }
}

pub fn new_future<T: 'static>(
    vtable: &'static FutureVtable<T>,
) -> (FutureWriter<T>, FutureReader<T>) {
    let handle = Stream::new();
    let handle2 = handle.clone(false);
    (
        FutureWriter::new(handle, vtable),
        FutureReader::new(handle2.take_handle() as *mut u8, vtable),
    )
}

pub unsafe fn future_new<T: 'static>(
    default: fn() -> T,
    vtable: &'static FutureVtable<T>,
) -> (FutureWriter<T>, FutureReader<T>) {
    let (mut writer, reader) = new_future(vtable);
    writer.default = Some(default);
    (writer, reader)
}
