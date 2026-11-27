use futures::{
    channel::oneshot,
    future::{AbortHandle, Abortable, Aborted, FusedFuture},
    task::Waker,
    FutureExt,
};
use std::{
    future::Future,
    mem::MaybeUninit,
    ops::DerefMut,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex, RwLock,
    },
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
mod futures_stream;
pub mod rust_buffer;
pub mod stream_support;
mod subtask;

struct FutureState<F: FusedFuture<Output = ()>> {
    future: F,
    // signal to activate once the current async future has finished
    completion_event: Option<EventGenerator>,
    // the event(s) this future should wake on (collected in `wake`),
    // and the means to wake the future by its `Waker`
    wake: Arc<WakeInner>,
    // number of times this future is registered for events
    instances: u32,
}

type StateContainer<F> = Mutex<FutureState<F>>;

/// State shared between a task and the `Waker`s handed out to its futures.
struct WakeInner {
    // the event(s) this future should wake on, turn to heapless for FuSa
    waiting_for: Mutex<Vec<EventSubscription>>,
    // activated by `wake`/`wake_by_ref`, the task registers on it
    // whenever someone could still wake it
    generator: EventGenerator,
    // set by `wake` since the last time the task was polled
    woken: AtomicBool,
    // references to this structure held by the executor itself
    internal_refs: AtomicUsize,
}

// The executor (and with it all tasks and wakers) is not shared between
// threads of the same task, the handles inside are plain integers.
unsafe impl Send for WakeInner {}
unsafe impl Sync for WakeInner {}

impl WakeInner {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            waiting_for: Mutex::new(Vec::new()),
            generator: EventGenerator::new(),
            woken: AtomicBool::new(false),
            internal_refs: AtomicUsize::new(0),
        })
    }

    fn wake(&self) {
        self.woken.store(true, Ordering::Release);
        self.generator.activate();
    }
}

static VTABLE: RawWakerVTable = RawWakerVTable::new(
    |data| {
        unsafe { Arc::increment_strong_count(data.cast::<WakeInner>()) };
        RawWaker::new(data, &VTABLE)
    },
    |data| {
        let inner = unsafe { Arc::from_raw(data.cast::<WakeInner>()) };
        inner.wake();
    },
    |data| unsafe { &*data.cast::<WakeInner>() }.wake(),
    |data| drop(unsafe { Arc::from_raw(data.cast::<WakeInner>()) }),
);

fn new_waker(wake: &Arc<WakeInner>) -> Waker {
    let data = Arc::into_raw(Arc::clone(wake));
    unsafe { Waker::from_raw(RawWaker::new(data.cast(), &VTABLE)) }
}

unsafe fn poll<F: FusedFuture<Output = ()>>(state: Pin<&mut FutureState<F>>) -> Poll<()> {
    let state_ref = unsafe { Pin::into_inner_unchecked(state) };
    if state_ref.future.is_terminated() {
        Poll::Ready(())
    } else {
        // pin projection
        let mut pinned = unsafe { Pin::new_unchecked(&mut state_ref.future) };
        // wakes from before this poll are served by it
        state_ref.wake.woken.store(false, Ordering::Release);
        let waker = new_waker(&state_ref.wake);
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
    let data = cx.waker().data();
    if core::ptr::eq(cx.waker().vtable(), &VTABLE) {
        unsafe { &*data.cast::<WakeInner>() }
            .waiting_for
            .lock()
            .unwrap()
            .push(wait_for);
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
            // registrations on the wake event are left, let them fire
            // so the state gets freed
            if state_inner.instances > 0 {
                state_inner.wake.generator.activate();
            }
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
            let mut events_to_register_to =
                std::mem::take(&mut *state_inner.wake.waiting_for.lock().unwrap());
            // someone holding a `Waker` can wake us at any time (or already did)
            let woken = state_inner.wake.woken.load(Ordering::Acquire);
            // (the state owns one reference)
            let held_elsewhere = Arc::strong_count(&state_inner.wake)
                > 1 + state_inner.wake.internal_refs.load(Ordering::Acquire);
            if woken || held_elsewhere {
                let subscription = state_inner.wake.generator.subscribe();
                // only wakes after this point count
                subscription.reset();
                events_to_register_to.push(subscription);
            }
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
            state_inner.instances += events_to_register_to.len() as u32;
            // keep it alive in case the task finishes while registering
            let wake = Arc::clone(&state_inner.wake);
            wake.internal_refs.fetch_add(1, Ordering::AcqRel);
            drop(state_inner);
            // now the mutex is unlocked
            for waiting_for in events_to_register_to.drain(..) {
                if super::register_unique(waiting_for, symmetric_callback::<F>, obj).is_none() {
                    #[cfg(feature = "trace")]
                    println!("already registered");
                    let mut state_inner = unsafe { &mut *state }.lock().unwrap();
                    state_inner.instances -= 1;
                }
            }
            // wake after registering, to not call back directly (which would recurse
            // endlessly for futures which yield in a loop)
            if woken {
                wake.generator.activate();
            }
            wake.internal_refs.fetch_sub(1, Ordering::AcqRel);
            #[cfg(feature = "trace")]
            println!(" chain {:x?}", wait_chain);
            wait_chain
        }
    }
}

extern "C" fn symmetric_callback<F: FusedFuture<Output = ()>>(obj: *mut ()) -> CallbackState {
    let wait_on = symmetric_callback_sub::<F>(obj);
    // drop the subscription as it's no longer needed
    if !wait_on.is_null() {
        let _ = unsafe { EventSubscription::from_handle(wait_on as usize) };
    }
    // obj already re-registered on new eventby _sub, stop calling
    // from the old event
    CallbackState::Ready
}

pub fn first_poll_sub<F: FusedFuture<Output = ()>>(future: F) -> *mut () {
    // Pin on the Box is assumed here
    let state = Box::into_raw(Box::new(Mutex::new(FutureState {
        future,
        completion_event: None,
        wake: WakeInner::new(),
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
///
/// The import is called right away, the returned future does not hold on to
/// `function` (which captures raw pointers) to stay `Send`.
#[doc(hidden)]
pub unsafe fn await_result(function: impl Fn() -> *mut u8) -> impl Future<Output = ()> + Send {
    let wait_for = function();
    let wait_for =
        (!wait_for.is_null()).then(|| unsafe { EventSubscription::from_handle(wait_for as usize) });
    async move {
        if let Some(wait_for) = wait_for {
            wait_on(wait_for).await;
        }
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

/// Spawn the future on the executor, it is polled right away and then
/// whenever the events it waits for (or its waker) fire.
///
/// Like in the canonical ABI runtime the result can be awaited using the
/// returned [`JoinHandle`], dropping the handle detaches the task.
pub fn spawn_local<T: 'static>(future: impl Future<Output = T> + 'static) -> JoinHandle<T> {
    let (sender, receiver) = oneshot::channel();
    let (abort, registration) = AbortHandle::new_pair();
    unsafe {
        spawn_unchecked(async move {
            let _ = sender.send(Abortable::new(future, registration).await);
        });
    }
    JoinHandle { receiver, abort }
}

/// A handle to a spawned task which can be awaited for its result.
///
/// Dropping the handle allows the task to continue running in the background.
/// Awaiting the handle returns `None` if the task was cancelled or otherwise
/// terminated without producing a result.
pub struct JoinHandle<T> {
    receiver: oneshot::Receiver<Result<T, Aborted>>,
    abort: AbortHandle,
}

impl<T> JoinHandle<T> {
    /// Cancels the spawned task. It is possible the task resolved and produced
    /// a result before cancelation completed and the [`JoinHandle`] can still
    /// be awaited to check for that case.
    pub fn cancel(&mut self) {
        self.abort.abort();
    }
}

impl<T> Future for JoinHandle<T> {
    type Output = Option<T>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
        match Pin::new(&mut self.receiver).poll(cx) {
            Poll::Ready(Ok(Ok(result))) => Poll::Ready(Some(result)),
            Poll::Ready(Ok(Err(_)) | Err(_)) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Gives other tasks the chance to progress, then continues.
pub async fn yield_async() {
    let mut yielded = false;
    std::future::poll_fn(move |cx| {
        if yielded {
            Poll::Ready(())
        } else {
            yielded = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    })
    .await
}

/// Blocking counterpart of [`yield_async`]. There is no other thread of control
/// to switch to here, so this only reports that the task was not cancelled.
pub fn yield_blocking() -> bool {
    true
}

pub fn block_on<T>(future: impl Future<Output = T>) -> T {
    // ugly but might do the trick
    let result: Arc<RwLock<MaybeUninit<T>>> = Arc::new(RwLock::new(MaybeUninit::uninit()));
    let result2 = Arc::clone(&result);
    let future2 = async move {
        let vec = future.await;
        result2.write().unwrap().write(vec);
    };
    // SAFETY: we don't return before the future is finished, so it can't
    // outlive the borrows it contains (same as the canonical `block_on`).
    // The future is never moved to another thread.
    let future2: Pin<Box<dyn Future<Output = ()> + '_>> = Box::pin(future2);
    let future2: Pin<Box<dyn Future<Output = ()> + 'static>> =
        unsafe { std::mem::transmute(future2) };
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
