// Generated by `wit-bindgen` 0.36.0. DO NOT EDIT!
// Options used:
#[allow(dead_code, clippy::all)]
pub mod test {
  pub mod test {

    #[allow(dead_code, clippy::all)]
    pub mod stream_source {
      #[used]
      #[doc(hidden)]
      static __FORCE_SECTION_REF: fn() =
      super::super::super::__link_custom_section_describing_imports;
      
      use super::super::super::_rt;

      impl _rt::stream_and_future_support::StreamPayload for u32{
        fn new() -> u32 {
          #[cfg(not(target_arch = "wasm32"))]
          {
            unreachable!();
          }

          #[cfg(target_arch = "wasm32")]
          {
            #[link(wasm_import_module = "[import-payload]test:test/stream-source")]
            extern "C" {
              #[link_name = "[stream-new-0]create"]
              fn new() -> u32;
            }
            unsafe { new() }
          }
        }

        async fn write(stream: u32, values: &[Self]) -> Option<usize> {
          #[cfg(not(target_arch = "wasm32"))]
          {
            unreachable!();
          }

          #[cfg(target_arch = "wasm32")]
          {
            let address = values.as_ptr() as _;
            
            #[link(wasm_import_module = "[import-payload]test:test/stream-source")]
            extern "C" {
              #[link_name = "[async][stream-write-0]create"]
              fn wit_import(_: u32, _: *mut u8, _: u32) -> u32;
            }

            unsafe {
              ::wit_bindgen_rt::async_support::await_stream_result(wit_import, stream, address, u32::try_from(values.len()).unwrap()).await
            }
          }
        }

        async fn read(stream: u32, values: &mut [::core::mem::MaybeUninit::<Self>]) -> Option<usize> {
          #[cfg(not(target_arch = "wasm32"))]
          {
            unreachable!();
          }

          #[cfg(target_arch = "wasm32")]
          {
            let address = values.as_mut_ptr() as _;
            #[link(wasm_import_module = "[import-payload]test:test/stream-source")]
            extern "C" {
              #[link_name = "[async][stream-read-0]create"]
              fn wit_import(_: u32, _: *mut u8, _: u32) -> u32;
            }

            let count = unsafe {
              ::wit_bindgen_rt::async_support::await_stream_result(wit_import, stream, address, u32::try_from(values.len()).unwrap()).await
            };
            #[allow(unused)]
            if let Some(count) = count {
              let value = ();

            }
            count
          }
        }

        fn cancel_write(writer: u32) {
          #[cfg(not(target_arch = "wasm32"))]
          {
            unreachable!();
          }

          #[cfg(target_arch = "wasm32")]
          {
            #[link(wasm_import_module = "[import-payload]test:test/stream-source")]
            extern "C" {
              #[link_name = "[stream-cancel-write-0]create"]
              fn cancel(_: u32) -> u32;
            }
            unsafe { cancel(writer) };
          }
        }

        fn cancel_read(reader: u32) {
          #[cfg(not(target_arch = "wasm32"))]
          {
            unreachable!();
          }

          #[cfg(target_arch = "wasm32")]
          {
            #[link(wasm_import_module = "[import-payload]test:test/stream-source")]
            extern "C" {
              #[link_name = "[stream-cancel-read-0]create"]
              fn cancel(_: u32) -> u32;
            }
            unsafe { cancel(reader) };
          }
        }

        fn close_writable(writer: u32) {
          #[cfg(not(target_arch = "wasm32"))]
          {
            unreachable!();
          }

          #[cfg(target_arch = "wasm32")]
          {
            #[link(wasm_import_module = "[import-payload]test:test/stream-source")]
            extern "C" {
              #[link_name = "[stream-close-writable-0]create"]
              fn drop(_: u32, _: u32);
            }
            unsafe { drop(writer, 0) }
          }
        }

        fn close_readable(reader: u32) {
          #[cfg(not(target_arch = "wasm32"))]
          {
            unreachable!();
          }

          #[cfg(target_arch = "wasm32")]
          {
            #[link(wasm_import_module = "[import-payload]test:test/stream-source")]
            extern "C" {
              #[link_name = "[stream-close-readable-0]create"]
              fn drop(_: u32);
            }
            unsafe { drop(reader) }
          }
        }
      }

      #[allow(unused_unsafe, clippy::all)]
      pub async fn create() -> _rt::stream_and_future_support::StreamReader<u32>{
        unsafe {
          let layout0 = _rt::alloc::Layout::from_size_align_unchecked(0, 1);
          let ptr0 = _rt::alloc::alloc(layout0);
          let layout1 = _rt::alloc::Layout::from_size_align_unchecked(core::mem::size_of::<*const u8>(), core::mem::size_of::<*const u8>());
          let ptr1 = _rt::alloc::alloc(layout1);

          #[link(wasm_import_module = "test:test/stream-source")]
          #[link(name="source")]
          extern "C" {
            #[cfg_attr(target_arch = "wasm32", link_name = "[async]create")]
            fn testX3AtestX2Fstream_sourceX00X5BasyncX5Dcreate(_: *mut u8, _: *mut u8, ) -> i32;
          }
          let layout2 = _rt::alloc::Layout::from_size_align_unchecked(0, 1);
          ::wit_bindgen_rt::async_support::await_result(testX3AtestX2Fstream_sourceX00X5BasyncX5Dcreate, layout2, ptr0, ptr1).await;
          let l3 = *ptr1.add(0).cast::<*mut u8>();
          let result4 = _rt::stream_and_future_support::StreamReader::from_handle(l3 as u32);
          _rt::cabi_dealloc(ptr1, core::mem::size_of::<*const u8>(), core::mem::size_of::<*const u8>());
          result4
        }
      }

    }

  }
}
#[allow(dead_code, clippy::all)]
pub mod exports {
  pub mod test {
    pub mod test {

      #[allow(dead_code, clippy::all)]
      pub mod stream_test {
        #[used]
        #[doc(hidden)]
        static __FORCE_SECTION_REF: fn() =
        super::super::super::super::__link_custom_section_describing_imports;
        
        use super::super::super::super::_rt;
        #[doc(hidden)]
        #[allow(non_snake_case)]
        pub unsafe fn _export_create_cabi<T: Guest>() -> *mut u8 {#[cfg(target_arch="wasm32")]
        _rt::run_ctors_once();let result0 = T::create();
        let result = ::wit_bindgen_rt::async_support::first_poll(result0, |result1| {
          

          #[link(wasm_import_module = "[export]test:test/stream-test")]
          extern "C" {
            #[cfg_attr(target_arch = "wasm32", link_name = "[task-return]create")]
            fn X5BexportX5DtestX3AtestX2Fstream_testX00X5Btask_returnX5Dcreate(_: i32, );
          }
          // X5BexportX5DtestX3AtestX2Fstream_testX00X5Btask_returnX5Dcreate((result1).into_handle() as i32);
        });

        result
      }
      #[doc(hidden)]
      #[allow(non_snake_case)]
      pub unsafe fn __callback_create(ctx: *mut u8, event0: i32, event1: i32, event2: i32) -> i32 {
        ::wit_bindgen_rt::async_support::callback(ctx, event0, event1, event2)
      }
      pub trait Guest {
        fn create() -> impl ::core::future::Future<Output = _rt::stream_and_future_support::StreamReader<u32>> + 'static;
      }
      #[doc(hidden)]

      macro_rules! __export_test_test_stream_test_cabi{
        ($ty:ident with_types_in $($path_to_types:tt)*) => (const _: () = {

          #[cfg_attr(target_arch = "wasm32", export_name = "create")]
          #[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
          unsafe extern "C" fn testX3AtestX2Fstream_testX00X5BasyncX5Dcreate(args: *mut u8, results: *mut u8) -> *mut u8 {
            $($path_to_types)*::_export_create_cabi::<$ty>()
          }
          #[export_name = "[callback]create"]
          unsafe extern "C" fn _callback_create(ctx: *mut u8, event0: i32, event1: i32, event2: i32) -> i32 {
            $($path_to_types)*::__callback_create(ctx, event0, event1, event2)
          }
        };);
      }
      #[doc(hidden)]
      pub(crate) use __export_test_test_stream_test_cabi;

    }

  }
}
}
mod _rt {
  #![allow(dead_code, clippy::all)]
  pub mod stream_and_future_support {use {
      futures::{
        channel::oneshot,
        future::{self, FutureExt},
        sink::Sink,
        stream::Stream,
      },
      std::{
        collections::hash_map::Entry,
        convert::Infallible,
        fmt,
        future::{Future, IntoFuture},
        iter,
        marker::PhantomData,
        mem::{self, ManuallyDrop, MaybeUninit},
        pin::Pin,
        task::{Context, Poll},
      },
      wit_bindgen_rt::async_support::{self, Handle},
    };

    #[doc(hidden)]
    pub trait FuturePayload: Unpin + Sized + 'static {
      fn new() -> u32;
      async fn write(future: u32, value: Self) -> bool;
      async fn read(future: u32) -> Option<Self>;
      fn cancel_write(future: u32);
      fn cancel_read(future: u32);
      fn close_writable(future: u32);
      fn close_readable(future: u32);
    }

    /// Represents the writable end of a Component Model `future`.
    pub struct FutureWriter<T: FuturePayload> {
      handle: u32,
      _phantom: PhantomData<T>,
    }

    impl<T: FuturePayload> fmt::Debug for FutureWriter<T> {
      fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FutureWriter")
        .field("handle", &self.handle)
        .finish()
      }
    }

    /// Represents a write operation which may be canceled prior to completion.
    pub struct CancelableWrite<T: FuturePayload> {
      writer: Option<FutureWriter<T>>,
      future: Pin<Box<dyn Future<Output = ()>>>,
    }

    impl<T: FuturePayload> Future for CancelableWrite<T> {
      type Output = ();

      fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<()> {
        let me = self.get_mut();
        match me.future.poll_unpin(cx) {
          Poll::Ready(()) => {
            me.writer = None;
            Poll::Ready(())
          }
          Poll::Pending => Poll::Pending,
        }
      }
    }

    impl<T: FuturePayload> CancelableWrite<T> {
      /// Cancel this write if it hasn't already completed, returning the original `FutureWriter`.
      ///
      /// This method will panic if the write has already completed.
      pub fn cancel(mut self) -> FutureWriter<T> {
        self.cancel_mut()
      }

      fn cancel_mut(&mut self) -> FutureWriter<T> {
        let writer = self.writer.take().unwrap();
        async_support::with_entry(writer.handle, |entry| match entry {
          Entry::Vacant(_) => unreachable!(),
          Entry::Occupied(mut entry) => match entry.get() {
            Handle::LocalOpen
            | Handle::LocalWaiting(_)
            | Handle::Read
            | Handle::LocalClosed => unreachable!(),
            Handle::LocalReady(..) => {
              entry.insert(Handle::LocalOpen);
            }
            Handle::Write => T::cancel_write(writer.handle),
          },
        });
        writer
      }
    }

    impl<T: FuturePayload> Drop for CancelableWrite<T> {
      fn drop(&mut self) {
        if self.writer.is_some() {
          self.cancel_mut();
        }
      }
    }

    impl<T: FuturePayload> FutureWriter<T> {
      /// Write the specified value to this `future`.
      pub fn write(self, v: T) -> CancelableWrite<T> {
        let handle = self.handle;
        CancelableWrite {
          writer: Some(self),
          future: async_support::with_entry(handle, |entry| match entry {
            Entry::Vacant(_) => unreachable!(),
            Entry::Occupied(mut entry) => match entry.get() {
              Handle::LocalOpen => {
                let mut v = Some(v);
                Box::pin(future::poll_fn(move |cx| {
                  async_support::with_entry(handle, |entry| match entry {
                    Entry::Vacant(_) => unreachable!(),
                    Entry::Occupied(mut entry) => match entry.get() {
                      Handle::LocalOpen => {
                        entry.insert(Handle::LocalReady(
                        Box::new(v.take().unwrap()),
                        cx.waker().clone(),
                        ));
                        Poll::Pending
                      }
                      Handle::LocalReady(..) => Poll::Pending,
                      Handle::LocalClosed => Poll::Ready(()),
                      Handle::LocalWaiting(_) | Handle::Read | Handle::Write => {
                        unreachable!()
                      }
                    },
                  })
                })) as Pin<Box<dyn Future<Output = _>>>
              }
              Handle::LocalWaiting(_) => {
                let Handle::LocalWaiting(tx) = entry.insert(Handle::LocalClosed) else {
                  unreachable!()
                };
                _ = tx.send(Box::new(v));
                Box::pin(future::ready(()))
              }
              Handle::LocalClosed => Box::pin(future::ready(())),
              Handle::Read | Handle::LocalReady(..) => unreachable!(),
              Handle::Write => Box::pin(T::write(handle, v).map(drop)),
            },
          }),
        }
      }
    }

    impl<T: FuturePayload> Drop for FutureWriter<T> {
      fn drop(&mut self) {
        async_support::with_entry(self.handle, |entry| match entry {
          Entry::Vacant(_) => unreachable!(),
          Entry::Occupied(mut entry) => match entry.get_mut() {
            Handle::LocalOpen | Handle::LocalWaiting(_) | Handle::LocalReady(..) => {
              entry.insert(Handle::LocalClosed);
            }
            Handle::Read => unreachable!(),
            Handle::Write | Handle::LocalClosed => {
              entry.remove();
              T::close_writable(self.handle);
            }
          },
        });
      }
    }

    /// Represents a read operation which may be canceled prior to completion.
    pub struct CancelableRead<T: FuturePayload> {
      reader: Option<FutureReader<T>>,
      future: Pin<Box<dyn Future<Output = Option<T>>>>,
    }

    impl<T: FuturePayload> Future for CancelableRead<T> {
      type Output = Option<T>;

      fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<T>> {
        let me = self.get_mut();
        match me.future.poll_unpin(cx) {
          Poll::Ready(v) => {
            me.reader = None;
            Poll::Ready(v)
          }
          Poll::Pending => Poll::Pending,
        }
      }
    }

    impl<T: FuturePayload> CancelableRead<T> {
      /// Cancel this read if it hasn't already completed, returning the original `FutureReader`.
      ///
      /// This method will panic if the read has already completed.
      pub fn cancel(mut self) -> FutureReader<T> {
        self.cancel_mut()
      }

      fn cancel_mut(&mut self) -> FutureReader<T> {
        let reader = self.reader.take().unwrap();
        async_support::with_entry(reader.handle, |entry| match entry {
          Entry::Vacant(_) => unreachable!(),
          Entry::Occupied(mut entry) => match entry.get() {
            Handle::LocalOpen
            | Handle::LocalReady(..)
            | Handle::Write
            | Handle::LocalClosed => unreachable!(),
            Handle::LocalWaiting(_) => {
              entry.insert(Handle::LocalOpen);
            }
            Handle::Read => T::cancel_read(reader.handle),
          },
        });
        reader
      }
    }

    impl<T: FuturePayload> Drop for CancelableRead<T> {
      fn drop(&mut self) {
        if self.reader.is_some() {
          self.cancel_mut();
        }
      }
    }

    /// Represents the readable end of a Component Model `future`.
    pub struct FutureReader<T: FuturePayload> {
      handle: u32,
      _phantom: PhantomData<T>,
    }

    impl<T: FuturePayload> fmt::Debug for FutureReader<T> {
      fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FutureReader")
        .field("handle", &self.handle)
        .finish()
      }
    }

    impl<T: FuturePayload> FutureReader<T> {
      #[doc(hidden)]
      pub fn from_handle(handle: u32) -> Self {
        async_support::with_entry(handle, |entry| match entry {
          Entry::Vacant(entry) => {
            entry.insert(Handle::Read);
          }
          Entry::Occupied(mut entry) => match entry.get() {
            Handle::Write => {
              entry.insert(Handle::LocalOpen);
            }
            Handle::Read
            | Handle::LocalOpen
            | Handle::LocalReady(..)
            | Handle::LocalWaiting(_)
            | Handle::LocalClosed => {
              unreachable!()
            }
          },
        });

        Self {
          handle,
          _phantom: PhantomData,
        }
      }

      #[doc(hidden)]
      pub fn into_handle(self) -> u32 {
        async_support::with_entry(self.handle, |entry| match entry {
          Entry::Vacant(_) => unreachable!(),
          Entry::Occupied(mut entry) => match entry.get() {
            Handle::LocalOpen => {
              entry.insert(Handle::Write);
            }
            Handle::Read | Handle::LocalClosed => {
              entry.remove();
            }
            Handle::LocalReady(..) | Handle::LocalWaiting(_) | Handle::Write => unreachable!(),
          },
        });

        ManuallyDrop::new(self).handle
      }
    }

    impl<T: FuturePayload> IntoFuture for FutureReader<T> {
      type Output = Option<T>;
      type IntoFuture = CancelableRead<T>;

      /// Convert this object into a `Future` which will resolve when a value is
      /// written to the writable end of this `future` (yielding a `Some` result)
      /// or when the writable end is dropped (yielding a `None` result).
      fn into_future(self) -> Self::IntoFuture {
        let handle = self.handle;
        CancelableRead {
          reader: Some(self),
          future: async_support::with_entry(handle, |entry| match entry {
            Entry::Vacant(_) => unreachable!(),
            Entry::Occupied(mut entry) => match entry.get() {
              Handle::Write | Handle::LocalWaiting(_) => unreachable!(),
              Handle::Read => Box::pin(async move { T::read(handle).await })
              as Pin<Box<dyn Future<Output = _>>>,
              Handle::LocalOpen => {
                let (tx, rx) = oneshot::channel();
                entry.insert(Handle::LocalWaiting(tx));
                Box::pin(async move { rx.await.ok().map(|v| *v.downcast().unwrap()) })
              }
              Handle::LocalClosed => Box::pin(future::ready(None)),
              Handle::LocalReady(..) => {
                let Handle::LocalReady(v, waker) = entry.insert(Handle::LocalClosed) else {
                  unreachable!()
                };
                waker.wake();
                Box::pin(future::ready(Some(*v.downcast().unwrap())))
              }
            },
          }),
        }
      }
    }

    impl<T: FuturePayload> Drop for FutureReader<T> {
      fn drop(&mut self) {
        async_support::with_entry(self.handle, |entry| match entry {
          Entry::Vacant(_) => unreachable!(),
          Entry::Occupied(mut entry) => match entry.get_mut() {
            Handle::LocalReady(..) => {
              let Handle::LocalReady(_, waker) = entry.insert(Handle::LocalClosed) else {
                unreachable!()
              };
              waker.wake();
            }
            Handle::LocalOpen | Handle::LocalWaiting(_) => {
              entry.insert(Handle::LocalClosed);
            }
            Handle::Read | Handle::LocalClosed => {
              entry.remove();
              T::close_readable(self.handle);
            }
            Handle::Write => unreachable!(),
          },
        });
      }
    }

    #[doc(hidden)]
    pub trait StreamPayload: Unpin + Sized + 'static {
      fn new() -> u32;
      async fn write(stream: u32, values: &[Self]) -> Option<usize>;
      async fn read(stream: u32, values: &mut [MaybeUninit<Self>]) -> Option<usize>;
      fn cancel_write(stream: u32);
      fn cancel_read(stream: u32);
      fn close_writable(stream: u32);
      fn close_readable(stream: u32);
    }

    struct CancelWriteOnDrop<T: StreamPayload> {
      handle: Option<u32>,
      _phantom: PhantomData<T>,
    }

    impl<T: StreamPayload> Drop for CancelWriteOnDrop<T> {
      fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
          async_support::with_entry(handle, |entry| match entry {
            Entry::Vacant(_) => unreachable!(),
            Entry::Occupied(mut entry) => match entry.get() {
              Handle::LocalOpen
              | Handle::LocalWaiting(_)
              | Handle::Read
              | Handle::LocalClosed => unreachable!(),
              Handle::LocalReady(..) => {
                entry.insert(Handle::LocalOpen);
              }
              Handle::Write => T::cancel_write(handle),
            },
          });
        }
      }
    }

    /// Represents the writable end of a Component Model `stream`.
    pub struct StreamWriter<T: StreamPayload> {
      handle: u32,
      future: Option<Pin<Box<dyn Future<Output = ()> + 'static>>>,
      _phantom: PhantomData<T>,
    }

    impl<T: StreamPayload> StreamWriter<T> {
      /// Cancel the current pending write operation.
      ///
      /// This will panic if no such operation is pending.
      pub fn cancel(&mut self) {
        assert!(self.future.is_some());
        self.future = None;
      }
    }

    impl<T: StreamPayload> fmt::Debug for StreamWriter<T> {
      fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StreamWriter")
        .field("handle", &self.handle)
        .finish()
      }
    }

    impl<T: StreamPayload> Sink<Vec<T>> for StreamWriter<T> {
      type Error = Infallible;

      fn poll_ready(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        let me = self.get_mut();

        if let Some(future) = &mut me.future {
          match future.as_mut().poll(cx) {
            Poll::Ready(_) => {
              me.future = None;
              Poll::Ready(Ok(()))
            }
            Poll::Pending => Poll::Pending,
          }
        } else {
          Poll::Ready(Ok(()))
        }
      }

      fn start_send(self: Pin<&mut Self>, item: Vec<T>) -> Result<(), Self::Error> {
        assert!(self.future.is_none());
        async_support::with_entry(self.handle, |entry| match entry {
          Entry::Vacant(_) => unreachable!(),
          Entry::Occupied(mut entry) => match entry.get() {
            Handle::LocalOpen => {
              let handle = self.handle;
              let mut item = Some(item);
              let mut cancel_on_drop = Some(CancelWriteOnDrop::<T> {
                handle: Some(handle),
                _phantom: PhantomData,
              });
              self.get_mut().future = Some(Box::pin(future::poll_fn(move |cx| {
                async_support::with_entry(handle, |entry| match entry {
                  Entry::Vacant(_) => unreachable!(),
                  Entry::Occupied(mut entry) => match entry.get() {
                    Handle::LocalOpen => {
                      if let Some(item) = item.take() {
                        entry.insert(Handle::LocalReady(
                        Box::new(item),
                        cx.waker().clone(),
                        ));
                        Poll::Pending
                      } else {
                        cancel_on_drop.take().unwrap().handle = None;
                        Poll::Ready(())
                      }
                    }
                    Handle::LocalReady(..) => Poll::Pending,
                    Handle::LocalClosed => {
                      cancel_on_drop.take().unwrap().handle = None;
                      Poll::Ready(())
                    }
                    Handle::LocalWaiting(_) | Handle::Read | Handle::Write => {
                      unreachable!()
                    }
                  },
                })
              })));
            }
            Handle::LocalWaiting(_) => {
              let Handle::LocalWaiting(tx) = entry.insert(Handle::LocalOpen) else {
                unreachable!()
              };
              _ = tx.send(Box::new(item));
            }
            Handle::LocalClosed => (),
            Handle::Read | Handle::LocalReady(..) => unreachable!(),
            Handle::Write => {
              let handle = self.handle;
              let mut cancel_on_drop = CancelWriteOnDrop::<T> {
                handle: Some(handle),
                _phantom: PhantomData,
              };
              self.get_mut().future = Some(Box::pin(async move {
                let mut offset = 0;
                while offset < item.len() {
                  if let Some(count) = T::write(handle, &item[offset..]).await {
                    offset += count;
                  } else {
                    break;
                  }
                }
                cancel_on_drop.handle = None;
                drop(cancel_on_drop);
              }));
            }
          },
        });
        Ok(())
      }

      fn poll_flush(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        self.poll_ready(cx)
      }

      fn poll_close(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), Self::Error>> {
        self.poll_ready(cx)
      }
    }

    impl<T: StreamPayload> Drop for StreamWriter<T> {
      fn drop(&mut self) {
        self.future = None;

        async_support::with_entry(self.handle, |entry| match entry {
          Entry::Vacant(_) => unreachable!(),
          Entry::Occupied(mut entry) => match entry.get_mut() {
            Handle::LocalOpen | Handle::LocalWaiting(_) | Handle::LocalReady(..) => {
              entry.insert(Handle::LocalClosed);
            }
            Handle::Read => unreachable!(),
            Handle::Write | Handle::LocalClosed => {
              entry.remove();
              T::close_writable(self.handle);
            }
          },
        });
      }
    }

    struct CancelReadOnDrop<T: StreamPayload> {
      handle: Option<u32>,
      _phantom: PhantomData<T>,
    }

    impl<T: StreamPayload> Drop for CancelReadOnDrop<T> {
      fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
          async_support::with_entry(handle, |entry| match entry {
            Entry::Vacant(_) => unreachable!(),
            Entry::Occupied(mut entry) => match entry.get() {
              Handle::LocalOpen
              | Handle::LocalReady(..)
              | Handle::Write
              | Handle::LocalClosed => unreachable!(),
              Handle::LocalWaiting(_) => {
                entry.insert(Handle::LocalOpen);
              }
              Handle::Read => T::cancel_read(handle),
            },
          });
        }
      }
    }

    /// Represents the readable end of a Component Model `stream`.
    pub struct StreamReader<T: StreamPayload> {
      handle: u32,
      future: Option<Pin<Box<dyn Future<Output = Option<Vec<T>>> + 'static>>>,
      _phantom: PhantomData<T>,
    }

    impl<T: StreamPayload> StreamReader<T> {
      /// Cancel the current pending read operation.
      ///
      /// This will panic if no such operation is pending.
      pub fn cancel(&mut self) {
        assert!(self.future.is_some());
        self.future = None;
      }
    }

    impl<T: StreamPayload> fmt::Debug for StreamReader<T> {
      fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StreamReader")
        .field("handle", &self.handle)
        .finish()
      }
    }

    impl<T: StreamPayload> StreamReader<T> {
      #[doc(hidden)]
      pub fn from_handle(handle: u32) -> Self {
        async_support::with_entry(handle, |entry| match entry {
          Entry::Vacant(entry) => {
            entry.insert(Handle::Read);
          }
          Entry::Occupied(mut entry) => match entry.get() {
            Handle::Write => {
              entry.insert(Handle::LocalOpen);
            }
            Handle::Read
            | Handle::LocalOpen
            | Handle::LocalReady(..)
            | Handle::LocalWaiting(_)
            | Handle::LocalClosed => {
              unreachable!()
            }
          },
        });

        Self {
          handle,
          future: None,
          _phantom: PhantomData,
        }
      }

      #[doc(hidden)]
      pub fn into_handle(self) -> u32 {
        async_support::with_entry(self.handle, |entry| match entry {
          Entry::Vacant(_) => unreachable!(),
          Entry::Occupied(mut entry) => match entry.get() {
            Handle::LocalOpen => {
              entry.insert(Handle::Write);
            }
            Handle::Read | Handle::LocalClosed => {
              entry.remove();
            }
            Handle::LocalReady(..) | Handle::LocalWaiting(_) | Handle::Write => unreachable!(),
          },
        });

        ManuallyDrop::new(self).handle
      }
    }

    impl<T: StreamPayload> Stream for StreamReader<T> {
      type Item = Vec<T>;

      fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let me = self.get_mut();

        if me.future.is_none() {
          me.future = Some(async_support::with_entry(me.handle, |entry| match entry {
            Entry::Vacant(_) => unreachable!(),
            Entry::Occupied(mut entry) => match entry.get() {
              Handle::Write | Handle::LocalWaiting(_) => unreachable!(),
              Handle::Read => {
                let handle = me.handle;
                let mut cancel_on_drop = CancelReadOnDrop::<T> {
                  handle: Some(handle),
                  _phantom: PhantomData,
                };
                Box::pin(async move {
                  let mut buffer = iter::repeat_with(MaybeUninit::uninit)
                  .take(ceiling(64 * 1024, mem::size_of::<T>()))
                  .collect::<Vec<_>>();

                  let result = if let Some(count) = T::read(handle, &mut buffer).await {
                    buffer.truncate(count);
                    Some(unsafe {
                      mem::transmute::<Vec<MaybeUninit<T>>, Vec<T>>(buffer)
                    })
                  } else {
                    None
                  };
                  cancel_on_drop.handle = None;
                  drop(cancel_on_drop);
                  result
                }) as Pin<Box<dyn Future<Output = _>>>
              }
              Handle::LocalOpen => {
                let (tx, rx) = oneshot::channel();
                entry.insert(Handle::LocalWaiting(tx));
                let mut cancel_on_drop = CancelReadOnDrop::<T> {
                  handle: Some(me.handle),
                  _phantom: PhantomData,
                };
                Box::pin(async move {
                  let result = rx.map(|v| v.ok().map(|v| *v.downcast().unwrap())).await;
                  cancel_on_drop.handle = None;
                  drop(cancel_on_drop);
                  result
                })
              }
              Handle::LocalClosed => Box::pin(future::ready(None)),
              Handle::LocalReady(..) => {
                let Handle::LocalReady(v, waker) = entry.insert(Handle::LocalOpen) else {
                  unreachable!()
                };
                waker.wake();
                Box::pin(future::ready(Some(*v.downcast().unwrap())))
              }
            },
          }));
        }

        match me.future.as_mut().unwrap().as_mut().poll(cx) {
          Poll::Ready(v) => {
            me.future = None;
            Poll::Ready(v)
          }
          Poll::Pending => Poll::Pending,
        }
      }
    }

    impl<T: StreamPayload> Drop for StreamReader<T> {
      fn drop(&mut self) {
        self.future = None;

        async_support::with_entry(self.handle, |entry| match entry {
          Entry::Vacant(_) => unreachable!(),
          Entry::Occupied(mut entry) => match entry.get_mut() {
            Handle::LocalReady(..) => {
              let Handle::LocalReady(_, waker) = entry.insert(Handle::LocalClosed) else {
                unreachable!()
              };
              waker.wake();
            }
            Handle::LocalOpen | Handle::LocalWaiting(_) => {
              entry.insert(Handle::LocalClosed);
            }
            Handle::Read | Handle::LocalClosed => {
              entry.remove();
              T::close_readable(self.handle);
            }
            Handle::Write => unreachable!(),
          },
        });
      }
    }

    /// Creates a new Component Model `future` with the specified payload type.
    pub fn new_future<T: FuturePayload>() -> (FutureWriter<T>, FutureReader<T>) {
      let handle = T::new();
      async_support::with_entry(handle, |entry| match entry {
        Entry::Vacant(entry) => {
          entry.insert(Handle::LocalOpen);
        }
        Entry::Occupied(_) => unreachable!(),
      });
      (
      FutureWriter {
        handle,
        _phantom: PhantomData,
      },
      FutureReader {
        handle,
        _phantom: PhantomData,
      },
      )
    }

    /// Creates a new Component Model `stream` with the specified payload type.
    pub fn new_stream<T: StreamPayload>() -> (StreamWriter<T>, StreamReader<T>) {
      let handle = T::new();
      async_support::with_entry(handle, |entry| match entry {
        Entry::Vacant(entry) => {
          entry.insert(Handle::LocalOpen);
        }
        Entry::Occupied(_) => unreachable!(),
      });
      (
      StreamWriter {
        handle,
        future: None,
        _phantom: PhantomData,
      },
      StreamReader {
        handle,
        future: None,
        _phantom: PhantomData,
      },
      )
    }

    fn ceiling(x: usize, y: usize) -> usize {
      (x / y) + if x % y == 0 { 0 } else { 1 }
    }
  }pub use alloc_crate::alloc;
  pub unsafe fn cabi_dealloc(ptr: *mut u8, size: usize, align: usize) {
    if size == 0 {
      return;
    }
    let layout = alloc::Layout::from_size_align_unchecked(size, align);
    alloc::dealloc(ptr, layout);
  }
  
  #[cfg(target_arch = "wasm32")]
  pub fn run_ctors_once() {
    wit_bindgen::rt::run_ctors_once();
  }
  extern crate alloc as alloc_crate;
}
#[allow(unused_imports)]
pub use _rt::stream_and_future_support;

/// Generates `#[no_mangle]` functions to export the specified type as the
/// root implementation of all generated traits.
///
/// For more information see the documentation of `wit_bindgen::generate!`.
///
/// ```rust
/// # macro_rules! export{ ($($t:tt)*) => (); }
/// # trait Guest {}
/// struct MyType;
///
/// impl Guest for MyType {
///     // ...
/// }
///
/// export!(MyType);
/// ```
#[allow(unused_macros)]
#[doc(hidden)]

macro_rules! __export_stream_world_impl {
  ($ty:ident) => (self::export!($ty with_types_in self););
  ($ty:ident with_types_in $($path_to_types_root:tt)*) => (
  $($path_to_types_root)*::exports::test::test::stream_test::__export_test_test_stream_test_cabi!($ty with_types_in $($path_to_types_root)*::exports::test::test::stream_test);
  )
}
#[doc(inline)]
pub(crate) use __export_stream_world_impl as export;

#[cfg(target_arch = "wasm32")]
#[link_section = "component-type:wit-bindgen:0.36.0:test:test:stream-world:encoded world"]
#[doc(hidden)]
#[allow(clippy::octal_escapes)]
pub static __WIT_BINDGEN_COMPONENT_TYPE: [u8; 262] = *b"\
\0asm\x0d\0\x01\0\0\x19\x16wit-component-encoding\x04\0\x07\x83\x01\x01A\x02\x01\
A\x04\x01B\x03\x01fy\x01@\0\0\0\x04\0\x06create\x01\x01\x03\0\x17test:test/strea\
m-source\x05\0\x01B\x03\x01fy\x01@\0\0\0\x04\0\x06create\x01\x01\x04\0\x15test:t\
est/stream-test\x05\x01\x04\0\x16test:test/stream-world\x04\0\x0b\x12\x01\0\x0cs\
tream-world\x03\0\0\0G\x09producers\x01\x0cprocessed-by\x02\x0dwit-component\x07\
0.221.2\x10wit-bindgen-rust\x060.36.0";

#[inline(never)]
#[doc(hidden)]
pub fn __link_custom_section_describing_imports() {
  wit_bindgen::rt::maybe_link_cabi_realloc();
}

