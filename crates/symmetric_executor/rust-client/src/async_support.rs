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
        atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering},
        Arc, Mutex, MutexGuard, RwLock,
    },
    task::{Context, Poll, RawWaker, RawWakerVTable},
};

use crate::executor_import::symmetric::runtime::symmetric_executor::{
    self, CallbackRegistration, CallbackState, EventGenerator, EventSubscription,
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
    // `None` once the task completed or was cancelled
    future: Option<F>,
    // signal to activate once the current async future has finished
    completion_event: Option<EventGenerator>,
    // tasks of async exports: the caller cancels them through this (see
    // `cancel_task`), until the task ended
    cancel: Option<CancelEvent>,
    // whether `cancel` is set up when the task goes pending for the first time
    cancellable: bool,
    // the event(s) this future should wake on (collected in `wake`),
    // and the means to wake the future by its `Waker`
    wake: Arc<WakeInner>,
    // number of times this future is registered for events, plus the callbacks
    // which work with the state while it is unlocked (see `settle`)
    instances: u32,
    // the registrations of this task which can still be pending, they are
    // cancelled once the task ended
    registrations: Vec<CallbackRegistration>,
}

/// The generator linked to the completion event of a task (`link_cancel`), it is
/// activated by a caller which cancels the task.
struct CancelEvent {
    generator: EventGenerator,
    // not registered anywhere, tells whether the generator was activated
    check: EventSubscription,
    // whether the task is registered on the generator
    registered: bool,
}

impl CancelEvent {
    fn new(completion: &EventGenerator) -> Self {
        let generator = EventGenerator::new();
        completion.link_cancel(&generator);
        Self::from_generator(generator)
    }

    fn from_generator(generator: EventGenerator) -> Self {
        // the subscription sees activations from before its creation as well
        let check = generator.subscribe();
        Self {
            generator,
            check,
            registered: false,
        }
    }
}

/// The state of a task, the executor calls `symmetric_callback` with a pointer to it.
struct TaskState<F: FusedFuture<Output = ()>> {
    // callbacks which found `state` locked, i.e. which ran while the task was
    // polled or dropped (by a nested executor loop of `block_on` or `cancel_task`),
    // the next one holding the lock accounts for them
    deferred: AtomicU32,
    state: Mutex<FutureState<F>>,
}

type StateContainer<F> = TaskState<F>;

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
    let Some(future) = state_ref.future.as_mut() else {
        return Poll::Ready(());
    };
    if future.is_terminated() {
        return Poll::Ready(());
    }
    // pin projection
    let mut pinned = unsafe { Pin::new_unchecked(future) };
    // wakes from before this poll are served by it
    state_ref.wake.woken.store(false, Ordering::Release);
    let waker = new_waker(&state_ref.wake);
    let mut context = Context::from_waker(&waker);
    #[cfg(feature = "trace")]
    println!(" Poll wait cx {:x?}", &context as *const _ as usize,);
    pinned.as_mut().poll(&mut context)
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

/// Poll based counterpart of [`wait_on`], `Ready` if the event is active,
/// otherwise the task is registered to be polled when it fires.
pub(crate) fn poll_event(cx: &mut Context<'_>, wait_for: &EventSubscription) -> Poll<()> {
    if wait_for.ready() {
        Poll::Ready(())
    } else {
        context_set_wait(cx, wait_for.dup());
        Poll::Pending
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
    let task = unsafe { &*obj.cast::<StateContainer<F>>() };
    let Ok(mut state_inner) = task.state.try_lock() else {
        // This task is being polled (or dropped) right now further up the stack,
        // which ran the executor (`block_on`, `cancel_task`). It takes care of
        // this event once it is done.
        #[cfg(feature = "trace")]
        println!(" deferred");
        task.deferred.fetch_add(1, Ordering::AcqRel);
        return core::ptr::null_mut();
    };
    state_inner.instances -= 1;

    // Has the caller cancelled this task (see `cancel_task`)?
    let cancel_requested = state_inner.future.is_some()
        && state_inner
            .cancel
            .as_ref()
            .is_some_and(|cancel| cancel.check.ready());
    let result = if cancel_requested {
        #[cfg(feature = "trace")]
        println!(" cancelled");
        // Dropping the future drops the arguments and everything else it owns,
        // callbacks of this task which run meanwhile are deferred (locked).
        state_inner.future = None;
        if let Some(completion) = state_inner.completion_event.take() {
            completion.activate_cancelled();
        }
        Poll::Ready(())
    } else {
        let state_mut = state_inner.deref_mut();
        let pinned_state = unsafe { Pin::new_unchecked(state_mut) };
        unsafe { poll(pinned_state) }
    };
    // callbacks which ran during the poll
    let deferred = task.deferred.swap(0, Ordering::AcqRel);
    state_inner.instances -= deferred;

    match result {
        Poll::Ready(_) => {
            state_inner.future = None;
            // The results are written, the caller can continue.
            if let Some(waker) = state_inner.completion_event.take() {
                waker.activate();
            }
            // nobody can cancel any more, the registration is dropped by settle
            state_inner.cancel = None;
            #[cfg(feature = "trace")]
            println!(" ready");
            // drop the registrations, free the state
            settle(obj, task, state_inner);
            core::ptr::null_mut()
        }
        Poll::Pending => {
            let mut events_to_register_to =
                std::mem::take(&mut *state_inner.wake.waiting_for.lock().unwrap());
            // someone holding a `Waker` can wake us at any time (or already did),
            // events which were deferred during the poll need another poll
            let woken = state_inner.wake.woken.load(Ordering::Acquire) || deferred > 0;
            // (the state owns one reference)
            let held_elsewhere = Arc::strong_count(&state_inner.wake)
                > 1 + state_inner.wake.internal_refs.load(Ordering::Acquire);
            if woken || held_elsewhere {
                let subscription = state_inner.wake.generator.subscribe();
                // only wakes after this point count
                subscription.reset();
                events_to_register_to.push(subscription);
            }
            if state_inner.completion_event.is_none() {
                let completion = EventGenerator::new();
                if state_inner.cancellable && state_inner.cancel.is_none() {
                    state_inner.cancel = Some(CancelEvent::new(&completion));
                }
                state_inner.completion_event = Some(completion);
            }
            let wait_chain = state_inner
                .completion_event
                .as_ref()
                .unwrap()
                .subscribe()
                .take_handle() as *mut ();
            // be polled when the caller cancels this task
            if let Some(cancel) = state_inner.cancel.as_mut() {
                if !cancel.registered {
                    cancel.registered = true;
                    events_to_register_to.push(cancel.generator.subscribe());
                }
            }
            // registrations which fired meanwhile are gone
            state_inner.registrations.retain(|r| r.is_pending());
            // We want to register without holding the lock to enable direct recursion
            // on ready. This callback keeps an instance while doing so, the state stays
            // even if the task finishes meanwhile (on a direct callback).
            state_inner.instances += events_to_register_to.len() as u32 + 1;
            let wake = Arc::clone(&state_inner.wake);
            wake.internal_refs.fetch_add(1, Ordering::AcqRel);
            drop(state_inner);
            // now the mutex is unlocked
            let mut registrations = Vec::with_capacity(events_to_register_to.len());
            let mut duplicates = 0;
            for waiting_for in events_to_register_to.drain(..) {
                match super::register_unique(waiting_for, symmetric_callback::<F>, obj) {
                    Some(registration) => registrations.push(registration),
                    None => {
                        #[cfg(feature = "trace")]
                        println!("already registered");
                        duplicates += 1;
                    }
                }
            }
            // wake after registering, to not call back directly (which would recurse
            // endlessly for futures which yield in a loop)
            if woken {
                wake.generator.activate();
            }
            wake.internal_refs.fetch_sub(1, Ordering::AcqRel);
            drop(wake);
            let mut state_inner = task.state.lock().unwrap();
            state_inner.instances -= duplicates + 1;
            state_inner.registrations.extend(registrations);
            // the task can have finished meanwhile
            settle(obj, task, state_inner);
            #[cfg(feature = "trace")]
            println!(" chain {:x?}", wait_chain);
            wait_chain
        }
    }
}

/// Account for the callbacks which ran while the state was locked (`deferred`). Once
/// the task ended its registrations are cancelled (instead of waiting for their
/// events) and the state is freed when nothing refers to it any more.
///
/// `callback-registration.cancel` returns the data (`obj`) if the callback won't be
/// called, its instance is released here. Otherwise the callback ran (or runs on
/// this thread right now, further up the stack) and accounted for its instance on
/// entry, or deferred it (the cancel of a callback which runs on another thread
/// returns after it returned). The registrations are cancelled while the state is
/// unlocked: a callback running on another thread could wait for the lock.
fn settle<'a, F: FusedFuture<Output = ()>>(
    obj: *mut (),
    task: &'a TaskState<F>,
    mut state_inner: MutexGuard<'a, FutureState<F>>,
) {
    loop {
        state_inner.instances -= task.deferred.swap(0, Ordering::AcqRel);
        if state_inner.future.is_some() {
            return;
        }
        if state_inner.registrations.is_empty() {
            if state_inner.instances == 0 {
                drop(state_inner);
                #[cfg(feature = "trace")]
                println!(" state {:x?} dropped", obj);
                let _ = unsafe { Box::from_raw(obj.cast::<StateContainer<F>>()) };
            }
            // otherwise a callback further up the stack (registering) settles
            return;
        }
        let registrations = std::mem::take(&mut state_inner.registrations);
        // keep the state while unlocked
        state_inner.instances += 1;
        drop(state_inner);
        let mut released = 0;
        for registration in registrations {
            if super::unregister::<()>(registration).is_some() {
                released += 1;
            }
        }
        state_inner = task.state.lock().unwrap();
        state_inner.instances -= released + 1;
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

/// Create the state of a task and poll it for the first time. `completion` and
/// `cancel` are the generators the caller already knows (tasks started after
/// backpressure), `cancellable` tasks (of async exports) can be cancelled by the
/// caller.
fn new_task<F: FusedFuture<Output = ()>>(
    future: F,
    completion: Option<EventGenerator>,
    cancel: Option<EventGenerator>,
    cancellable: bool,
) -> *mut () {
    // Pin on the Box is assumed here
    let state = Box::into_raw(Box::new(TaskState {
        deferred: AtomicU32::new(0),
        state: Mutex::new(FutureState {
            future: Some(future),
            completion_event: completion,
            cancel: cancel.map(CancelEvent::from_generator),
            cancellable,
            wake: WakeInner::new(),
            instances: 1,
            registrations: Vec::new(),
        }),
    }));
    // the future needs to be pinned, but we need a mutex around it for later,
    // so we trade the mutex overhead for pin consistency
    symmetric_callback_sub::<F>(state.cast())
}

pub fn first_poll_sub<F: FusedFuture<Output = ()>>(future: F) -> *mut () {
    new_task(future, None, None, false)
}

/// Poll the future generated by a call to an async-lifted export once, calling
/// the specified closure (presumably backed by a call to `task.return`) when it
/// generates a value.
///
/// This will return a non-null pointer representing the task if it hasn't
/// completed immediately; otherwise it returns null.
#[doc(hidden)]
pub fn first_poll(future: impl Future<Output = ()> + 'static) -> *mut () {
    new_task(future.fuse(), None, None, true)
}

/// A task created while backpressure is active, it starts (is polled for the
/// first time) once it is released. The caller already got the subscription of
/// its completion and can cancel it before it starts.
struct QueuedTask {
    start: Box<dyn FnOnce(EventGenerator, EventGenerator)>,
    completion: EventGenerator,
    cancel: EventGenerator,
    // the registration of `cancel_queued_task`, set right after registering
    registration: Option<CallbackRegistration>,
}

/// The data of the `cancel_queued_task` registration
type QueuedData = Box<Arc<QueuedSlot>>;

/// The task started (or was cancelled by its registration further up the stack),
/// the registration for its cancellation before the start is not needed any more.
fn drop_queued_registration(registration: Option<CallbackRegistration>) {
    if let Some(data) = registration.and_then(super::unregister::<Arc<QueuedSlot>>) {
        drop(unsafe { QueuedData::from_raw(data) });
    }
}

// empty once the task started or was cancelled
type QueuedSlot = Mutex<Option<QueuedTask>>;

struct StartingTask(Arc<QueuedSlot>);

// tasks are only ever started by the thread which releases the backpressure,
// the same as everything else polling a task
unsafe impl Send for StartingTask {}

static BACKPRESSURE: AtomicUsize = AtomicUsize::new(0);
static STARTING: Mutex<Vec<StartingTask>> = Mutex::new(Vec::new());

#[doc(hidden)]
pub fn start_task(future: impl Future<Output = ()> + 'static) -> *mut u8 {
    let mut starting = STARTING.lock().unwrap();
    if BACKPRESSURE.load(Ordering::Acquire) == 0 {
        drop(starting);
        return first_poll(future).cast();
    }
    let completion = EventGenerator::new();
    let cancel = EventGenerator::new();
    completion.link_cancel(&cancel);
    let subscription = completion.subscribe();
    let cancel_subscription = cancel.subscribe();
    let slot = Arc::new(Mutex::new(Some(QueuedTask {
        start: Box::new(move |completion, cancel| {
            let wait_for = new_task(future.fuse(), Some(completion), Some(cancel), true);
            if !wait_for.is_null() {
                drop(unsafe { EventSubscription::from_handle(wait_for as usize) });
            }
        }),
        completion,
        cancel,
        registration: None,
    })));
    starting.push(StartingTask(Arc::clone(&slot)));
    drop(starting);
    // cancelled once the task starts
    let data = Box::into_raw(Box::new(Arc::clone(&slot)));
    let registration = super::register(cancel_subscription, cancel_queued_task, data);
    let mut queued = slot.lock().unwrap();
    match queued.as_mut() {
        Some(task) => task.registration = Some(registration),
        // started already (by a callback which ran while registering)
        None => {
            drop(queued);
            drop_queued_registration(Some(registration));
        }
    }
    subscription.take_handle() as *mut u8
}

/// The caller cancelled a task which is waiting for backpressure to be released
/// (or the task has started meanwhile, then there is nothing to do here).
extern "C" fn cancel_queued_task(data: *mut Arc<QueuedSlot>) -> CallbackState {
    let slot = unsafe { Box::from_raw(data) };
    let queued = slot.lock().unwrap().take();
    if let Some(QueuedTask {
        start,
        completion,
        cancel,
        // this registration, it ends with this callback
        registration: _,
    }) = queued
    {
        STARTING
            .lock()
            .unwrap()
            .retain(|task| !Arc::ptr_eq(&task.0, &slot));
        // this drops the future, and with it the arguments
        drop(start);
        completion.activate_cancelled();
        drop(cancel);
    }
    CallbackState::Ready
}

/// Delay the start of new tasks of exported async functions until
/// [`backpressure_dec`] was called as often. The caller of such a function
/// gets the completion event right away, the body of the function does not
/// run before it is started.
pub fn backpressure_inc() {
    let _starting = STARTING.lock().unwrap();
    BACKPRESSURE.fetch_add(1, Ordering::AcqRel);
}

/// See [`backpressure_inc`], starts the delayed tasks when it reaches zero.
pub fn backpressure_dec() {
    let tasks = {
        let mut starting = STARTING.lock().unwrap();
        let previous = BACKPRESSURE.fetch_sub(1, Ordering::AcqRel);
        assert!(previous > 0, "backpressure_dec without backpressure_inc");
        if previous == 1 {
            std::mem::take(&mut *starting)
        } else {
            Vec::new()
        }
    };
    for StartingTask(slot) in tasks {
        // a cancelled task is gone already
        let queued = slot.lock().unwrap().take();
        if let Some(QueuedTask {
            start,
            completion,
            cancel,
            registration,
        }) = queued
        {
            drop_queued_registration(registration);
            start(completion, cancel);
        }
    }
}

/// Await the completion of a call to an async-lowered import.
///
/// The import is called right away, the returned future does not hold on to
/// `function` (which captures raw pointers) to stay `Send`.
///
/// Dropping the returned future while the call is still in progress cancels the
/// call (`cancel_task`, which returns once the callee no longer accesses the
/// arguments and the results), like the canonical ABI does.
#[doc(hidden)]
pub unsafe fn await_result(function: impl Fn() -> *mut u8) -> impl Future<Output = ()> + Send {
    let wait_for = function();
    let task =
        (!wait_for.is_null()).then(|| unsafe { EventSubscription::from_handle(wait_for as usize) });
    AwaitResult { task }
}

/// The call of an async import, see [`await_result`].
struct AwaitResult {
    // the completion of the callee, `None` once it completed
    task: Option<EventSubscription>,
}

impl Future for AwaitResult {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        match &self.task {
            None => Poll::Ready(()),
            Some(task) => {
                if task.ready() {
                    self.task = None;
                    Poll::Ready(())
                } else {
                    #[cfg(feature = "trace")]
                    println!("await_result sub {:x?} pending", task.handle());
                    context_set_wait(cx, task.dup());
                    Poll::Pending
                }
            }
        }
    }
}

impl Drop for AwaitResult {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            // returns right away if the call has completed meanwhile
            let _cancelled = symmetric_executor::cancel_task(task);
        }
    }
}

pub fn spawn(future: impl Future<Output = ()> + 'static + Send) {
    // nobody can cancel it
    let wait_for = first_poll_sub(future.fuse());
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
    let wait_for = first_poll_sub(future2.fuse());
    if !wait_for.is_null() {
        let wait_for = unsafe { EventSubscription::from_handle(wait_for as usize) };
        symmetric_executor::block_on(wait_for);
    }
    return unsafe { result.to_owned().write().unwrap().assume_init_read() };
}

/// Counterpart of the guard of the canonical ABI runtime, which calls
/// `task.cancel` when the future of an async export is dropped before it returned.
///
/// The symmetric ABI has no `task.return`: the generated code writes the results
/// right before the future ends, and never calls [`TaskCancelOnDrop::forget`].
/// Cancellation is handled by the task itself instead (see `start_task`): it drops
/// the future and reports the end with `activate_cancelled`. So this does nothing.
pub struct TaskCancelOnDrop;

impl TaskCancelOnDrop {
    pub fn new() -> Self {
        Self
    }
    pub fn forget(self) {}
}

pub unsafe fn callback(_event0: u32, _event1: u32, _event2: u32) -> u32 {
    todo!();
}
