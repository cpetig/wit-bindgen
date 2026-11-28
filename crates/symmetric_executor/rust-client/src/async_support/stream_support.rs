pub use crate::stream_import::symmetric::runtime::symmetric_stream::StreamObj as Stream;
use crate::{
    async_support::{poll_event, rust_buffer::RustBuffer},
    symmetric_stream::{Address, Buffer, StreamState},
    Cleanup,
};
use {
    futures::sink::Sink,
    std::{
        alloc::Layout,
        fmt,
        future::Future,
        mem,
        pin::Pin,
        task::{Context, Poll},
    },
};

// waitable::{WaitableOp, WaitableOperation} looked cool, but
// as it waits for the runtime and uses Wakers I don't think the
// logic fits

#[doc(hidden)]
pub struct StreamVtable<T> {
    pub layout: Layout,
    pub lower: Option<unsafe fn(value: T, dst: *mut u8)>,
    pub lift: Option<unsafe fn(dst: *mut u8) -> T>,
    //pub dealloc_lists: Option<unsafe fn(dst: *mut u8)>,
}

// const fn ceiling(x: usize, y: usize) -> usize {
//     (x / y) + if x % y == 0 { 0 } else { 1 }
// }

pub mod results {
    pub const BLOCKED: isize = -1;
    pub const CLOSED: isize = isize::MIN;
    pub const CANCELED: isize = 0;
}

// Used within Waitable
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum StreamResult {
    Complete(usize),
    Dropped,
    Cancelled,
}

/// Memory holding `count` lowered elements which are offered to the stream.
/// It has to stay valid until the operation completed or was cancelled.
struct Lowered {
    ptr: *mut u8,
    count: usize,
    _cleanup: Option<Cleanup>,
}

// the pointer is only used by the owner (and the stream while offered)
unsafe impl Send for Lowered {}

impl Lowered {
    fn new(layout: Layout, count: usize) -> Self {
        let (ptr, cleanup) =
            Cleanup::new(Layout::from_size_align(layout.size() * count, layout.align()).unwrap());
        Self {
            ptr,
            count,
            _cleanup: cleanup,
        }
    }

    fn buffer(&self, layout: Layout, size: usize) -> Buffer {
        let address = unsafe { Address::from_handle(self.ptr as usize) };
        let buffer = Buffer::new(address, self.count as u64, layout.size() as u64);
        buffer.set_size(size as u64);
        buffer
    }
}

/// Move `count` values to the (lowered) elements at `dst`.
unsafe fn lower_all<T>(
    vtable: &StreamVtable<T>,
    values: impl Iterator<Item = T>,
    mut dst: *mut u8,
) {
    for value in values {
        if let Some(lower) = vtable.lower {
            unsafe { (lower)(value, dst) };
        } else {
            // the element stride is the layout size, which is not the size of the
            // Rust type for e.g. `()` (zero sized in Rust, one byte in the ABI)
            unsafe { dst.cast::<T>().write(value) };
        }
        dst = unsafe { dst.byte_add(vtable.layout.size()) };
    }
}

/// Take ownership of `count` lowered elements at `src`.
unsafe fn lift_all<T>(vtable: &StreamVtable<T>, count: usize, mut src: *mut u8) -> Vec<T> {
    let mut result = Vec::with_capacity(count);
    for _ in 0..count {
        result.push(if let Some(lift) = vtable.lift {
            unsafe { (lift)(src) }
        } else {
            unsafe { src.cast::<T>().read() }
        });
        src = unsafe { src.byte_add(vtable.layout.size()) };
    }
    result
}

/// A write which has been offered to the stream, `values` is empty meanwhile
/// (the values were lowered).
struct WriteOp(Lowered);

pub struct StreamWrite<'a, T: 'static> {
    writer: &'a mut StreamWriter<T>,
    values: RustBuffer<T>,
    op: Option<WriteOp>,
}

impl<T: Unpin + Send + 'static> StreamWrite<'_, T> {
    /// Cancel the write. The result tells how many values were written
    /// (`Complete`) or that none were (`Cancelled`), the values which were
    /// not written are returned.
    pub fn cancel(self: Pin<&mut Self>) -> (StreamResult, RustBuffer<T>) {
        let me = self.get_mut();
        let result = me.writer.cancel_op(&mut me.op, &mut me.values);
        (
            result,
            mem::replace(&mut me.values, RustBuffer::new(Vec::new())),
        )
    }
}

impl<T: Unpin + Send + 'static> Future for StreamWrite<'_, T> {
    type Output = (StreamResult, RustBuffer<T>);

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.get_mut();
        me.writer
            .poll_write(cx, &mut me.values, &mut me.op)
            .map(|result| {
                (
                    result,
                    mem::replace(&mut me.values, RustBuffer::new(Vec::new())),
                )
            })
    }
}

impl<T> Drop for StreamWrite<'_, T> {
    fn drop(&mut self) {
        // take the offer back before the memory goes away
        if let Some(op) = self.op.take() {
            if let Some(buffer) = self.writer.handle.cancel_write() {
                let consumed = buffer.get_size() as usize;
                drop(unsafe {
                    lift_all(
                        self.writer._vtable,
                        op.0.count - consumed,
                        op.0.ptr
                            .byte_add(consumed * self.writer._vtable.layout.size()),
                    )
                });
            } else {
                // the offer is gone (reader closed), the elements are still ours
                drop(unsafe { lift_all(self.writer._vtable, op.0.count, op.0.ptr) });
            }
        }
    }
}

pub struct StreamWriter<T: 'static> {
    handle: Stream,
    _vtable: &'static StreamVtable<T>,
    // `Sink` state: the values to send and the write in flight
    sink: Option<(RustBuffer<T>, Option<WriteOp>)>,
}

impl<T> StreamWriter<T> {
    #[doc(hidden)]
    pub fn new(handle: Stream, vtable: &'static StreamVtable<T>) -> Self {
        Self {
            handle,
            _vtable: vtable,
            sink: None,
        }
    }

    // TODO: Make this non-blocking
    pub fn write(&mut self, values: Vec<T>) -> StreamWrite<'_, T> {
        self.write_buf(RustBuffer::new(values))
    }

    pub fn write_buf(&mut self, values: RustBuffer<T>) -> StreamWrite<'_, T> {
        StreamWrite {
            writer: self,
            values,
            op: None,
        }
    }

    /// Cancel the write started by the `Sink` implementation (if any), the
    /// values which were not written are kept.
    pub fn cancel(&mut self) {
        if let Some((mut values, mut op)) = self.sink.take() {
            self.cancel_op(&mut op, &mut values);
            self.sink = Some((values, op));
        }
    }

    // cancel a write in flight, the values which were not written are
    // put (back) into `values`
    fn cancel_op(&self, op: &mut Option<WriteOp>, values: &mut RustBuffer<T>) -> StreamResult {
        let Some(WriteOp(lowered)) = op.take() else {
            return StreamResult::Cancelled;
        };
        let consumed = match self.handle.cancel_write() {
            Some(buffer) => buffer.get_size() as usize,
            // the reader closed
            None => 0,
        };
        let rest = unsafe {
            lift_all(
                self._vtable,
                lowered.count - consumed,
                lowered.ptr.byte_add(consumed * self._vtable.layout.size()),
            )
        };
        *values = RustBuffer::new(rest);
        if consumed > 0 {
            StreamResult::Complete(consumed)
        } else {
            StreamResult::Cancelled
        }
    }

    // Drive a write of (the rest of) `values`. On completion the values which
    // were not written are in `values`.
    fn poll_write(
        &self,
        cx: &mut Context<'_>,
        values: &mut RustBuffer<T>,
        op: &mut Option<WriteOp>,
    ) -> Poll<StreamResult> {
        let vtable = self._vtable;
        if op.is_none() {
            if values.remaining() == 0 {
                // delayed flush
                // I assume EOF
                // hopefully the calling instance drops the stream for EOF signalling
                return Poll::Ready(StreamResult::Complete(0));
            }
            match self.handle.start_writing() {
                // a reader is waiting, write directly into its buffer
                Ok(buffer) => {
                    let addr = buffer.get_address().take_handle() as *mut u8;
                    let size = (buffer.capacity() as usize).min(values.remaining());
                    unsafe { lower_all(vtable, values.drain_n(size), addr) };
                    buffer.set_size(size as u64);
                    self.handle
                        .finish_writing(buffer)
                        .unwrap_or_else(|_| unreachable!("borrowed buffer rejected"));
                    return Poll::Ready(StreamResult::Complete(size));
                }
                // all readers closed, hand back the values
                Err(StreamState::Eof) => return Poll::Ready(StreamResult::Dropped),
                // we are first, offer all the values
                Err(StreamState::Pending) => {
                    let count = values.remaining();
                    let lowered = Lowered::new(vtable.layout, count);
                    unsafe { lower_all(vtable, values.drain_n(count), lowered.ptr) };
                    self.handle
                        .offer_write(lowered.buffer(vtable.layout, count))
                        .unwrap_or_else(|_| unreachable!("write already in flight"));
                    *op = Some(WriteOp(lowered));
                }
            }
        }
        loop {
            let lowered = &op.as_ref().unwrap().0;
            let subscription = self.handle.write_ready_subscribe();
            subscription.reset();
            match self.handle.write_result() {
                Ok(buffer) => {
                    // The write is over: the reader took everything, or (when
                    // `is-read-closed`) it took `consumed` elements before it
                    // went away. The runtime keeps the write pending (and
                    // offers the rest to later reads) until then, so there is
                    // nothing to offer again. We keep the elements it did
                    // not take.
                    let consumed = buffer.get_size() as usize;
                    let rest = unsafe {
                        lift_all(
                            vtable,
                            lowered.count - consumed,
                            lowered.ptr.byte_add(consumed * vtable.layout.size()),
                        )
                    };
                    *values = RustBuffer::new(rest);
                    *op = None;
                    return Poll::Ready(StreamResult::Complete(consumed));
                }
                Err(StreamState::Eof) => {
                    // all readers closed, hand back the values
                    let rest = unsafe { lift_all(vtable, lowered.count, lowered.ptr) };
                    *values = RustBuffer::new(rest);
                    *op = None;
                    return Poll::Ready(StreamResult::Dropped);
                }
                Err(StreamState::Pending) => {
                    if poll_event(cx, &subscription).is_pending() {
                        return Poll::Pending;
                    }
                }
            }
        }
    }

    #[cfg(feature = "trace")]
    pub fn take_handle(&self) -> usize {
        self.handle.take_handle()
    }
}

impl<T> Drop for StreamWriter<T> {
    fn drop(&mut self) {
        if let Some((mut values, mut op)) = self.sink.take() {
            self.cancel_op(&mut op, &mut values);
        }
    }
}

impl<T: Unpin + Send> StreamWriter<T> {
    pub async fn write_all(&mut self, values: Vec<T>) -> Vec<T> {
        let (mut status, mut buf) = self.write(values).await;

        while let StreamResult::Complete(_) = status {
            if buf.remaining() == 0 {
                break;
            }
            (status, buf) = self.write_buf(buf).await;

            // FIXME(WebAssembly/component-model#490)
            if status == StreamResult::Cancelled {
                status = StreamResult::Complete(0);
            }
        }
        assert!(buf.remaining() == 0 || matches!(status, StreamResult::Dropped));
        buf.into_vec()
    }

    pub async fn write_one(&mut self, value: T) -> Option<T> {
        self.write_all(std::vec![value]).await.pop()
    }
}

impl<T> fmt::Debug for StreamWriter<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StreamWriter")
            .field("handle", &self.handle)
            .finish()
    }
}

impl<T: Unpin + Send> Sink<Vec<T>> for StreamWriter<T> {
    type Error = (); // Eof

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        self.poll_flush(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: Vec<T>) -> Result<(), Self::Error> {
        let me = self.get_mut();
        assert!(me.sink.is_none(), "start_send without poll_ready");
        me.sink = Some((RustBuffer::new(item), None));
        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        let me = self.get_mut();
        while let Some((mut values, mut op)) = me.sink.take() {
            let result = me.poll_write(cx, &mut values, &mut op);
            match result {
                Poll::Pending => {
                    me.sink = Some((values, op));
                    return Poll::Pending;
                }
                Poll::Ready(StreamResult::Dropped) => return Poll::Ready(Err(())),
                Poll::Ready(_) => {
                    if values.remaining() > 0 {
                        me.sink = Some((values, op));
                    }
                }
            }
        }
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        self.poll_flush(cx)
    }
}

/// Represents the readable end of a Component Model `stream`.
pub struct StreamReader<T: 'static> {
    handle: Stream,
    vtable: &'static StreamVtable<T>,
}

impl<T> fmt::Debug for StreamReader<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StreamReader")
            .field("handle", &self.handle)
            .finish()
    }
}

impl<T> StreamReader<T> {
    #[doc(hidden)]
    pub unsafe fn new(handle: *mut u8, vtable: &'static StreamVtable<T>) -> Self {
        Self {
            handle: unsafe { Stream::from_handle(handle as usize) },
            vtable,
        }
    }

    pub unsafe fn from_handle(handle: *mut u8, vtable: &'static StreamVtable<T>) -> Self {
        Self::new(handle, vtable)
    }

    /// Cancel a read still offered to the stream (the future returned by
    /// [`StreamReader::read`] does that when it is dropped).
    pub fn cancel(&mut self) {
        let _ = self.handle.cancel_read();
    }

    #[doc(hidden)]
    pub fn take_handle(&self) -> usize {
        self.handle.take_handle()
    }

    #[doc(hidden)]
    // remove this as it is weirder than take_handle
    pub fn into_handle(self) -> *mut () {
        self.handle.take_handle() as *mut ()
    }

    pub fn read(&mut self, buf: Vec<T>) -> StreamRead<'_, T> {
        StreamRead {
            reader: self,
            buf,
            op: None,
        }
    }
}

impl<T: Send + Unpin + 'static> StreamReader<T> {
    /// Read all remaining items from this stream into a list and await the
    /// stream to be dropped.
    pub async fn collect(mut self) -> Vec<T> {
        let mut ret = Vec::new();
        loop {
            // reserve room, the reader fills at most the capacity
            if ret.len() == ret.capacity() {
                ret.reserve(1);
            }
            let (status, buf) = self.read(ret).await;
            ret = buf;
            match status {
                StreamResult::Complete(_) => {}
                StreamResult::Dropped | StreamResult::Cancelled => break,
            }
        }
        ret
    }

    pub async fn next(&mut self) -> Option<T> {
        let (status, mut buf) = self.read(Vec::with_capacity(1)).await;
        #[cfg(feature = "trace")]
        println!("self.read finished with status {:?}", status);
        match status {
            StreamResult::Complete(_) => buf.pop(),
            StreamResult::Dropped | StreamResult::Cancelled => None,
        }
    }
}

pub struct StreamRead<'a, T: 'static> {
    buf: Vec<T>,
    reader: &'a mut StreamReader<T>,
    // the buffer offered to the stream while the read is in flight
    op: Option<Lowered>,
}

impl<T: Unpin + Send + 'static> StreamRead<'_, T> {
    // lift `count` elements read into the offered buffer
    fn finish(&mut self, lowered: Lowered, count: usize) -> Vec<T> {
        let mut buf = mem::take(&mut self.buf);
        buf.extend(unsafe { lift_all(self.reader.vtable, count, lowered.ptr) });
        buf
    }

    /// Cancel the read. The result tells how many elements were read
    /// (`Complete`) or that none were (`Cancelled`).
    pub fn cancel(self: Pin<&mut Self>) -> (StreamResult, Vec<T>) {
        let me = self.get_mut();
        let mut result = StreamResult::Cancelled;
        if let Some(lowered) = me.op.take() {
            if let Some(buffer) = me.reader.handle.cancel_read() {
                let count = buffer.get_size() as usize;
                if count > 0 {
                    let buf = me.finish(lowered, count);
                    me.buf = buf;
                    result = StreamResult::Complete(count);
                }
            }
        }
        (result, mem::take(&mut me.buf))
    }
}

impl<T> Drop for StreamRead<'_, T> {
    fn drop(&mut self) {
        // take the offered buffer back before it is freed
        if let Some(lowered) = self.op.take() {
            if let Some(buffer) = self.reader.handle.cancel_read() {
                // elements already transferred are ours to drop
                drop(unsafe {
                    lift_all(self.reader.vtable, buffer.get_size() as usize, lowered.ptr)
                });
            }
        }
    }
}

impl<T: Unpin + Send + 'static> Future for StreamRead<'_, T> {
    type Output = (StreamResult, Vec<T>);

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.get_mut();

        if me.op.is_none() {
            if me.reader.handle.is_write_closed() {
                return Poll::Ready((StreamResult::Dropped, mem::take(&mut me.buf)));
            }
            let vtable = me.reader.vtable;
            let capacity = me.buf.capacity();
            let lowered = Lowered::new(vtable.layout, capacity);
            me.reader
                .handle
                .start_reading(lowered.buffer(vtable.layout, 0))
                .unwrap_or_else(|_| unreachable!("read already in flight"));
            me.op = Some(lowered);
        }
        loop {
            let subscription = me.reader.handle.read_ready_subscribe();
            subscription.reset();
            match me.reader.handle.read_result() {
                Ok(buffer) => {
                    let count = buffer.get_size() as usize;
                    let lowered = me.op.take().unwrap();
                    let buf = me.finish(lowered, count);
                    return Poll::Ready((StreamResult::Complete(count), buf));
                }
                Err(StreamState::Eof) => {
                    me.op = None;
                    return Poll::Ready((StreamResult::Dropped, mem::take(&mut me.buf)));
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

/// deprecate this, replace with stream_new
pub fn new_stream<T: 'static>(
    vtable: &'static StreamVtable<T>,
) -> (StreamWriter<T>, StreamReader<T>) {
    let handle = Stream::new();
    let handle2 = handle.clone(false);
    (StreamWriter::new(handle, vtable), unsafe {
        StreamReader::new(handle2.take_handle() as *mut u8, vtable)
    })
}

pub fn stream_new<T: 'static>(
    vtable: &'static StreamVtable<T>,
) -> (StreamWriter<T>, StreamReader<T>) {
    new_stream(vtable)
}
