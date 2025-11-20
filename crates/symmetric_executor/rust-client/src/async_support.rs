use futures::{future::FusedFuture, task::Waker, FutureExt};
use std::{
    future::Future,
    mem::MaybeUninit,
    ops::DerefMut,
    pin::Pin,
    sync::{Arc, Mutex, RwLock},
    task::{Context, Poll, RawWaker, RawWakerVTable},
};

use crate::executor_import::symmetric::runtime::symmetric_executor::{
    self, CallbackState, EventGenerator, EventSubscription,
};

pub use future_support::{future_new, FutureReader, FutureVtable, FutureWriteCancel, FutureWriter};
pub use stream_support::{
    results, stream_new, Stream, StreamReader, StreamResult, StreamVtable, StreamWriter,
};
pub use subtask::Subtask;

pub mod future_support;
pub mod rust_buffer;
pub mod stream_support;
mod subtask;

struct FutureState<F: FusedFuture<Output = ()>> {
    future: F,
    // signal to activate once the current async future has finished
    completion_event: Option<EventGenerator>,
    // the event(s) this future should wake on, turn to heapless for FuSa
    waiting_for: Vec<EventSubscription>,
    // number of times this future is registered for events
    instances: u32,
}

type StateContainer<F> = Mutex<FutureState<F>>;

static VTABLE: RawWakerVTable = RawWakerVTable::new(
    |_| RawWaker::new(core::ptr::null(), &VTABLE),
    // `wake` does nothing
    |_| {},
    // `wake_by_ref` does nothing
    |_| {},
    // Dropping does nothing as we don't allocate anything
    |_| {},
);

pub fn new_waker(waiting_for_ptr: *mut Vec<EventSubscription>) -> Waker {
    unsafe { Waker::from_raw(RawWaker::new(waiting_for_ptr.cast(), &VTABLE)) }
}

unsafe fn poll<F: FusedFuture<Output = ()>>(state: Pin<&mut FutureState<F>>) -> Poll<()> {
    let state_ref = unsafe { Pin::into_inner_unchecked(state) };
    if state_ref.future.is_terminated() {
        Poll::Ready(())
    } else {
        // pin projection
        let mut pinned = unsafe { Pin::new_unchecked(&mut state_ref.future) };
        let waker = new_waker(&mut state_ref.waiting_for as *mut Vec<EventSubscription>);
        let mut context = Context::from_waker(&waker);
        #[cfg(feature = "trace")]
        println!(" Poll wait cx {:x?}", &context as *const _ as usize,);
        pinned.as_mut().poll(&mut context)
    }
}

pub fn context_set_wait(cx: &Context, wait_for: EventSubscription) {
    // remember this eventsubscription in the context
    #[cfg(feature = "trace")]
    println!(
        "Set wait cx {:x?} sub {:x?}",
        cx as *const _ as usize,
        wait_for.handle()
    );
    let data = cx
        .waker()
        .data()
        .cast_mut()
        .cast::<Vec<EventSubscription>>();
    if !data.is_null() {
        unsafe { &mut *data }.push(wait_for);
    } else {
        println!("await in wrong context");
    }
}

pub async fn wait_on(wait_for: EventSubscription) {
    std::future::poll_fn(move |cx| {
        if wait_for.ready() {
            Poll::Ready(())
        } else {
            #[cfg(feature = "trace")]
            println!("wait_on sub {:x?} pending", wait_for.handle());
            context_set_wait(cx, wait_for.dup());
            Poll::Pending
        }
    })
    .await
}

// return new completion event on Pending
fn symmetric_callback_sub<F: FusedFuture<Output = ()>>(obj: *mut ()) -> *mut () {
    #[cfg(feature = "trace")]
    println!("# Callback on {:?}", obj);
    let state = obj.cast::<StateContainer<F>>();
    let mut state_inner = unsafe { &mut *state }.lock().unwrap();
    state_inner.instances -= 1;
    let state_mut = state_inner.deref_mut();
    let pinned_state = unsafe { Pin::new_unchecked(state_mut) };

    match unsafe { poll(pinned_state) } {
        Poll::Ready(_) => {
            // free obj if last instance finished
            if state_inner.instances == 0 {
                if let Some(waker) = &state_inner.completion_event {
                    waker.activate();
                }
                drop(state_inner);
                #[cfg(feature = "trace")]
                println!(" state {:x?} dropped", state);
                let _ = unsafe { Box::from_raw(state) };
            }
            #[cfg(feature = "trace")]
            println!(" ready");
            core::ptr::null_mut()
        }
        Poll::Pending => {
            assert!(!state_inner.waiting_for.is_empty() || state_inner.instances > 0);
            let wait_chain = if state_inner.completion_event.is_none() {
                state_inner
                    .completion_event
                    .insert(EventGenerator::new())
                    .subscribe()
                    .take_handle() as *mut ()
            } else {
                state_inner
                    .completion_event
                    .as_ref()
                    .unwrap()
                    .subscribe()
                    .take_handle() as *mut ()
                // core::ptr::null_mut()
            };
            // we want to register without holding the lock to enable direct recursion on ready
            let mut events_to_register_to = Vec::new();
            std::mem::swap(&mut state_inner.waiting_for, &mut events_to_register_to);
            state_inner.instances += events_to_register_to.len() as u32;
            drop(state_inner);
            // now the mutex is unlocked
            for waiting_for in events_to_register_to.drain(..) {
                super::register(waiting_for, symmetric_callback::<F>, obj);
            }
            #[cfg(feature = "trace")]
            println!(" chain {:x?}", wait_chain);
            wait_chain
        }
    }
}

extern "C" fn symmetric_callback<F: FusedFuture<Output = ()>>(obj: *mut ()) -> CallbackState {
    let _ = symmetric_callback_sub::<F>(obj);
    // obj already re-registered on new eventby _sub, stop calling
    // from the old event
    CallbackState::Ready
}

pub fn first_poll_sub<F: FusedFuture<Output = ()>>(future: F) -> *mut () {
    // Pin on the Box is assumed here
    let state = Box::into_raw(Box::new(Mutex::new(FutureState {
        future,
        completion_event: None,
        waiting_for: Vec::new(),
        instances: 1,
    })));
    // the future needs to be pinned, but we need a mutex around it for later,
    // so we trade the mutex overhead for pin consistency
    symmetric_callback_sub::<F>(state.cast())
}

/// Poll the future generated by a call to an async-lifted export once, calling
/// the specified closure (presumably backed by a call to `task.return`) when it
/// generates a value.
///
/// This will return a non-null pointer representing the task if it hasn't
/// completed immediately; otherwise it returns null.
#[doc(hidden)]
pub fn first_poll(future: impl Future<Output = ()> + 'static) -> *mut () {
    first_poll_sub(future.fuse())
}

#[doc(hidden)]
pub fn start_task(future: impl Future<Output = ()> + 'static) -> *mut u8 {
    first_poll(future).cast()
}

/// Await the completion of a call to an async-lowered import.
#[doc(hidden)]
pub async unsafe fn await_result(function: impl Fn() -> *mut u8) {
    let wait_for = function();
    if !wait_for.is_null() {
        let wait_for = unsafe { EventSubscription::from_handle(wait_for as usize) };
        wait_on(wait_for).await;
    }
}

pub fn spawn(future: impl Future<Output = ()> + 'static + Send) {
    let wait_for = first_poll(future);
    if !wait_for.is_null() {
        let wait_for = unsafe { EventSubscription::from_handle(wait_for as usize) };
        drop(wait_for);
    }
}

pub unsafe fn spawn_unchecked(future: impl Future<Output = ()>) {
    let wait_for = first_poll_sub(future.fuse());
    if !wait_for.is_null() {
        let wait_for = unsafe { EventSubscription::from_handle(wait_for as usize) };
        drop(wait_for);
    }
}

pub fn block_on<T: 'static>(future: impl Future<Output = T> + 'static) -> T {
    // ugly but might do the trick
    let result: Arc<RwLock<MaybeUninit<T>>> = Arc::new(RwLock::new(MaybeUninit::uninit()));
    let result2 = Arc::clone(&result);
    let future2 = async move {
        let vec = future.await;
        result2.write().unwrap().write(vec);
    };
    let wait_for = first_poll(future2);
    if !wait_for.is_null() {
        let wait_for = unsafe { EventSubscription::from_handle(wait_for as usize) };
        symmetric_executor::block_on(wait_for);
    }
    return unsafe { result.to_owned().write().unwrap().assume_init_read() };
}

pub struct TaskCancelOnDrop;

impl TaskCancelOnDrop {
    pub fn new() -> Self {
        // todo!();
        Self
    }
    pub fn forget(self) {}
}

pub unsafe fn callback(_event0: u32, _event1: u32, _event2: u32) -> u32 {
    todo!();
}
