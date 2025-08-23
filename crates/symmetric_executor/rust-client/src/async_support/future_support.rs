use std::{
    alloc::Layout,
    future::{Future, IntoFuture},
    mem::MaybeUninit,
    pin::Pin,
    task::{Context, Poll},
};

use futures::FutureExt;

use crate::symmetric_stream::{Address, Buffer};

use super::{super::Cleanup, wait_on, Stream};

#[doc(hidden)]
pub struct FutureVtable<T> {
    pub layout: Layout,
    pub lower: unsafe fn(value: T, dst: *mut u8),
    pub lift: unsafe fn(dst: *mut u8) -> T,
}

pub struct FutureWriter<T: 'static> {
    handle: Stream,
    vtable: &'static FutureVtable<T>,
}

impl<T> FutureWriter<T> {
    pub fn new(handle: Stream, vtable: &'static FutureVtable<T>) -> Self {
        Self { handle, vtable }
    }

    pub fn write(self, data: T) -> FutureWrite<T> {
        FutureWrite {
            writer: self,
            future: None,
            data: Some(data),
        }
    }
}

/// Represents a write operation which may be canceled prior to completion.
pub struct FutureWrite<T: 'static> {
    writer: FutureWriter<T>,
    future: Option<Pin<Box<dyn Future<Output = Result<(), FutureWriteError<T>>> + 'static + Send>>>,
    data: Option<T>,
}

#[derive(Debug)]
pub struct FutureWriteError<T> {
    pub value: T,
}

impl<T: Unpin + Send> Future for FutureWrite<T> {
    type Output = Result<(), FutureWriteError<T>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let me = self.get_mut();

        if me.future.is_none() {
            let handle = me.writer.handle.clone();
            let data = me.data.take().unwrap();
            let lower = me.writer.vtable.lower;
            me.future = Some(Box::pin(async move {
                if !handle.is_ready_to_write() {
                    let subsc = handle.write_ready_subscribe();
                    wait_on(subsc).await;
                }
                if handle.is_read_closed() {
                    Err(FutureWriteError { value: data })
                } else {
                    let buffer = handle.start_writing();
                    let addr = buffer.get_address().take_handle() as *mut MaybeUninit<T> as *mut u8;
                    unsafe { (lower)(data, addr) };
                    buffer.set_size(1);
                    handle.finish_writing(Some(buffer));
                    Ok(())
                }
            })
                as Pin<Box<dyn Future<Output = Self::Output> + Send>>);
        }
        me.future.as_mut().unwrap().poll_unpin(cx)
    }
}

impl<T> std::fmt::Debug for FutureWriter<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FutureWriter")
            .field("handle", &self.handle.handle())
            .finish()
    }
}

impl<T: 'static> FutureWrite<T> {
    pub fn cancel(self: Pin<&mut Self>) -> FutureWriteCancel<T> {
        todo!()
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
    reader: FutureReader<T>,
    future: Option<Pin<Box<dyn Future<Output = T> + 'static + Send>>>,
}

pub struct FutureReader<T: 'static> {
    handle: Stream,
    vtable: &'static FutureVtable<T>,
    has_completed: bool,
}

impl<T> FutureReader<T> {
    pub fn new(handle: *mut u8, vtable: &'static FutureVtable<T>) -> Self {
        Self {
            handle: unsafe { Stream::from_handle(handle as usize) },
            vtable,
            has_completed: false,
        }
    }

    pub fn read(self) -> FutureRead<T> {
        FutureRead {
            reader: self,
            future: None,
        }
    }

    pub unsafe fn from_handle(handle: *mut u8, vtable: &'static FutureVtable<T>) -> Self {
        Self::new(handle, vtable)
    }

    pub fn take_handle(&self) -> *mut () {
        self.handle.take_handle() as *mut ()
    }
}

impl<T> Drop for FutureReader<T> {
    fn drop(&mut self) {
        if !self.has_completed && self.handle.handle() != 0 {
            let _ = self.handle.close_read();
        }
    }
}

impl<T> std::fmt::Debug for FutureReader<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FutureReader")
            .field("handle", &self.handle.handle())
            .finish()
    }
}

impl<T: Unpin + Sized + Send> Future for FutureRead<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<T> {
        let me = self.get_mut();

        if me.future.is_none() {
            let handle = me.reader.handle.clone();
            let vtable = me.reader.vtable;
            me.future = Some(Box::pin(async move {
                // sadly there is no easy way to embed this in the future as the size is not accessible at compile time
                let (buffer0, cleanup) = Cleanup::new(vtable.layout);
                let address = unsafe { Address::from_handle(buffer0 as usize) };
                let buffer = Buffer::new(address, 1);
                handle.start_reading(buffer);
                let subsc = handle.read_ready_subscribe();
                subsc.reset();
                wait_on(subsc).await;
                let buffer2 = handle.read_result();
                if let Some(buffer2) = buffer2 {
                    let count = buffer2.get_size();
                    if count > 0 {
                        unsafe { (vtable.lift)(buffer2.get_address().take_handle() as *mut u8) }
                    } else {
                        // make sure it lives long enough
                        drop(cleanup);
                        todo!()
                    }
                } else {
                    todo!()
                }
            }) as Pin<Box<dyn Future<Output = _> + Send>>);
        }

        match me.future.as_mut().unwrap().as_mut().poll(cx) {
            Poll::Ready(v) => {
                me.future = None;
                me.reader.has_completed = true;
                Poll::Ready(v)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<T> FutureRead<T> {
    pub fn cancel(self: Pin<&mut Self>) -> Result<T, FutureReader<T>> {
        let me = self.get_mut();
        let mut local_me = FutureRead { reader: FutureReader { handle: Stream::new(), vtable: me.reader.vtable, has_completed: false }, future: None };
        std::mem::swap(me, &mut local_me);
        let FutureRead{ reader, future } = local_me;

        let buffer2 = reader.handle.read_result();
        let res = if let Some(buffer2) = buffer2 {
            let count = buffer2.get_size();
            if count > 0 {
                Ok(unsafe { (reader.vtable.lift)(buffer2.get_address().take_handle() as *mut u8) })
            } else {
                Err(reader)
            }
        } else {
            Err(reader)
        };
        if future.is_some() {
            // deregister future callback
        }
        res
    }

    // fn cancel_mut(&mut self) -> Result<T, FutureReader<T>> {
    //     todo!()
    // }
}

impl<T: Send + Unpin + Sized> IntoFuture for FutureReader<T> {
    type Output = T;
    type IntoFuture = FutureRead<T>;

    /// Convert this object into a `Future` which will resolve when a value is
    /// written to the writable end of this `future` (yielding a `Some` result)
    /// or when the writable end is dropped (yielding a `None` result).
    fn into_future(self) -> Self::IntoFuture {
        self.read()
    }
}

pub fn new_future<T: 'static>(
    vtable: &'static FutureVtable<T>,
) -> (FutureWriter<T>, FutureReader<T>) {
    let handle = Stream::new();
    let handle2 = handle.clone();
    (
        FutureWriter::new(handle, vtable),
        FutureReader::new(handle2.take_handle() as *mut u8, vtable),
    )
}

pub unsafe fn future_new<T: 'static>(
    _default: fn() -> T,
    vtable: &'static FutureVtable<T>,
) -> (FutureWriter<T>, FutureReader<T>) {
    new_future(vtable)
    // let handle = Stream::new();
    // let handle2 = handle.clone();
    // (
    //     FutureWriter::new(handle, vtable),
    //     FutureReader::new(handle2, vtable),
    // )
}
