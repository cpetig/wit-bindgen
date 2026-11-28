use std::{
    collections::VecDeque,
    mem::transmute,
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        Arc, Condvar, Mutex,
    },
    thread::ThreadId,
    time::{Duration, SystemTime},
};

use executor::exports::symmetric::runtime::symmetric_executor::{
    self, CallbackState, GuestCallbackRegistration,
};

const DEBUGGING: bool = cfg!(feature = "trace");
const INVALID_FD: EventFd = -1;

mod executor;

struct Guest;

executor::export!(Guest with_types_in executor);

fn gettid() -> libc::pid_t {
    #[cfg(feature = "trace")]
    unsafe {
        libc::gettid()
    }
    #[cfg(not(feature = "trace"))]
    0
}

struct Ignore;
struct OpaqueData;
impl symmetric_executor::GuestCallbackFunction for Ignore {}
impl symmetric_executor::GuestCallbackData for OpaqueData {}

// Hide the specifics of eventfd
mod event_fd {
    pub type EventFd = core::ffi::c_int;
    use super::gettid;

    pub fn activate(fd: EventFd) {
        let file_signal: u64 = 1;
        if super::DEBUGGING {
            println!("{} activate(fd {fd})", gettid());
        }

        let result = unsafe {
            libc::write(
                fd,
                core::ptr::from_ref(&file_signal).cast(),
                core::mem::size_of_val(&file_signal),
            )
        };
        if result >= 0 {
            assert_eq!(
                result,
                core::mem::size_of_val(&file_signal).try_into().unwrap()
            );
        }
    }
    pub fn consume(fd: EventFd) {
        let mut dummy: u64 = 0;

        let readresult = unsafe {
            libc::read(
                fd,
                core::ptr::from_mut(&mut dummy).cast(),
                core::mem::size_of_val(&dummy),
            )
        };
        assert!(
            readresult <= 0
                || readresult == isize::try_from(core::mem::size_of_val(&dummy)).unwrap()
        );
    }
}

use event_fd::EventFd;

struct WaitSet {
    wait: libc::timeval,
    // non null if timeval is finite (timeout)
    tvptr: *mut libc::timeval,
    maxfd: EventFd,
    rfds: core::mem::MaybeUninit<libc::fd_set>,
}

struct WaitSetIterator<'a> {
    ws: &'a WaitSet,
    fd: EventFd,
}

impl<'a> Iterator for WaitSetIterator<'a> {
    type Item = EventFd;

    fn next(&mut self) -> Option<Self::Item> {
        let rfd_ptr = self.ws.rfds.as_ptr();
        while self.fd < self.ws.maxfd {
            let fd = self.fd;
            self.fd += 1;
            if unsafe { libc::FD_ISSET(fd, rfd_ptr) } {
                return Some(fd);
            }
        }
        None
    }
}

impl WaitSet {
    fn new(change_event: Option<EventFd>) -> Self {
        let wait = libc::timeval {
            tv_sec: i64::MAX,
            tv_usec: 999999,
        };
        let tvptr = core::ptr::null_mut();
        let maxfd = change_event.map_or(0, |fd| fd + 1);
        let mut rfds = core::mem::MaybeUninit::<libc::fd_set>::uninit();
        let rfd_ptr = rfds.as_mut_ptr();
        unsafe { libc::FD_ZERO(rfd_ptr) };
        if let Some(fd) = change_event {
            unsafe {
                libc::FD_SET(fd, rfd_ptr);
            }
        }
        Self {
            wait,
            tvptr,
            maxfd,
            rfds,
        }
    }

    fn register(&mut self, fd: EventFd) {
        let rfd_ptr = self.rfds.as_mut_ptr();
        unsafe { libc::FD_SET(fd, rfd_ptr) };
        if fd >= self.maxfd {
            self.maxfd = fd + 1;
        }
    }

    fn timeout(&mut self, diff: Duration) {
        let secs = diff.as_secs() as i64;
        let usecs = diff.subsec_micros() as i64;
        if secs < self.wait.tv_sec || (secs == self.wait.tv_sec && usecs < self.wait.tv_usec) {
            self.wait.tv_sec = secs;
            self.wait.tv_usec = usecs;
        }
        self.tvptr = core::ptr::from_mut(&mut self.wait);
    }

    fn debug(&self) {
        let rfd_ptr = self.rfds.as_ptr();
        if self.tvptr.is_null() {
            println!("{} select({}, {:x}, null)", gettid(), self.maxfd, unsafe {
                *rfd_ptr.cast::<u32>()
            },);
        } else {
            println!(
                "{} select({}, {:x}, {}.{})",
                gettid(),
                self.maxfd,
                unsafe { *rfd_ptr.cast::<u32>() },
                self.wait.tv_sec,
                self.wait.tv_usec
            );
        }
    }

    // see select for the return value
    fn wait(&mut self) -> i32 {
        let rfd_ptr = self.rfds.as_mut_ptr();
        unsafe {
            libc::select(
                self.maxfd,
                rfd_ptr,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                self.tvptr,
            )
        }
    }

    fn iter_active(&self) -> WaitSetIterator<'_> {
        WaitSetIterator { ws: self, fd: 0 }
    }
}

/// The handle returned by `register`, `None` if the callback completed during
/// `register` already
struct CallbackRegistrationInternal(Option<Arc<Registration>>);

impl symmetric_executor::GuestEventSubscription for EventSubscriptionInternal {
    fn ready(&self) -> bool {
        self.inner.ready()
    }

    fn from_timeout(nanoseconds: u64) -> symmetric_executor::EventSubscription {
        let when = SystemTime::now() + Duration::from_nanos(nanoseconds);
        symmetric_executor::EventSubscription::new(EventSubscriptionInternal {
            inner: EventType::SystemTime(when),
        })
    }

    fn dup(&self) -> symmetric_executor::EventSubscription {
        let res = symmetric_executor::EventSubscription::new(self.dup());
        // to avoid endless recursion de-activate the original
        // self.reset();
        res
    }

    fn reset(&self) {
        match &self.inner {
            EventType::Triggered {
                last_counter,
                event,
            } => {
                let current = event.lock().unwrap().counter;
                let last = last_counter.load(Ordering::Relaxed);
                if last != current && DEBUGGING {
                    println!(
                        "{} counter reset {}->{} {:x}",
                        gettid(),
                        last,
                        current,
                        self as *const Self as usize
                    );
                }
                last_counter.store(current, Ordering::Relaxed);
            }
            EventType::SystemTime(_system_time) => (),
        }
    }
}

impl symmetric_executor::GuestEventGenerator for EventGenerator {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(EventInner {
            counter: 0,
            waiting: Default::default(),
            cancel: None,
            cancelled: false,
        })))
    }

    fn subscribe(&self) -> symmetric_executor::EventSubscription {
        if DEBUGGING {
            println!(
                "{} subscribe({:x})",
                gettid(),
                Arc::as_ptr(&self.0) as usize
            );
        }
        symmetric_executor::EventSubscription::new(EventSubscriptionInternal {
            inner: EventType::Triggered {
                last_counter: AtomicU32::new(0),
                event: Arc::clone(&self.0),
            },
        })
    }

    fn activate(&self) {
        activate_event(&self.0, false);
    }

    fn link_cancel(&self, cancel: symmetric_executor::EventGeneratorBorrow<'_>) {
        let cancel = Arc::clone(&cancel.get::<EventGenerator>().0);
        if DEBUGGING {
            println!(
                "{} link_cancel({:x}, {:x})",
                gettid(),
                Arc::as_ptr(&self.0) as usize,
                Arc::as_ptr(&cancel) as usize
            );
        }
        self.0.lock().unwrap().cancel = Some(cancel);
    }

    fn activate_cancelled(&self) {
        activate_event(&self.0, true);
    }
}

/// Trigger all subscribers of the event, `cancelled` marks the end of a task
/// because of a cancellation (see `cancel_task`).
fn activate_event(event: &Arc<Mutex<EventInner>>, cancelled: bool) {
    if let Ok(mut event_inner) = event.lock() {
        event_inner.counter += 1;
        if cancelled {
            event_inner.cancelled = true;
        }
        if DEBUGGING {
            println!(
                "{} activate({:x}) counter={}{}",
                gettid(),
                Arc::as_ptr(event) as usize,
                event_inner.counter,
                if cancelled { " cancelled" } else { "" }
            );
        }
        event_inner.waiting.iter().for_each(|fd| {
            event_fd::activate(*fd);
        });
    } else if DEBUGGING {
        println!("{} activate failure", gettid());
    }
}

impl GuestCallbackRegistration for CallbackRegistrationInternal {
    fn cancel(
        obj: symmetric_executor::CallbackRegistration,
    ) -> Option<symmetric_executor::CallbackData> {
        let obj: CallbackRegistrationInternal = obj.into_inner();
        let reg = obj.0?;
        let data = reg.cancel()?;
        // the registration is no longer queued, remove it (with its eventfd) from
        // the lists, a tick which has it in its hands right now drops it instead
        // of queueing it again
        let task = {
            let mut tasks = TASKS.lock().unwrap();
            match tasks.iter().position(|task| Arc::ptr_eq(&task.reg, &reg)) {
                Some(pos) => tasks.remove(pos),
                None => {
                    let mut new_tasks = NEW_TASKS.lock().unwrap();
                    new_tasks
                        .iter()
                        .position(|task| Arc::ptr_eq(&task.reg, &reg))
                        .map(|pos| new_tasks.swap_remove(pos))
                }
            }
        };
        if DEBUGGING {
            println!(
                "{} cancel registration {:x} {:x} found={}",
                gettid(),
                reg.callback.0 as usize,
                data as usize,
                task.is_some()
            );
        }
        if let Some(task) = task {
            // dropped without holding a lock
            drop(task);
            // a thread waiting for the events of the remaining tasks can see that
            // there are none left
            wake_executor();
        }
        Some(unsafe { symmetric_executor::CallbackData::from_handle(data as usize) })
    }

    fn is_pending(&self) -> bool {
        self.0.as_ref().is_some_and(|reg| reg.is_pending())
    }
}

/// Where a registered callback is in its life cycle
#[derive(Clone, Copy, PartialEq, Debug)]
enum Phase {
    /// Waiting for the event (in TASKS, NEW_TASKS or checked by a tick)
    Queued,
    /// The callback runs on this thread
    Running(ThreadId),
    /// The callback runs on this thread and was cancelled meanwhile, it is not
    /// called again
    RunningCancelled(ThreadId),
    /// The callback returned ready or was cancelled
    Done,
}

struct RegistrationState {
    phase: Phase,
    // whether the callback is listed in REGISTRATIONS
    registered: bool,
}

/// The state of a registration, shared between its task (`QueuedEvent`) and its
/// handle (`callback-registration`).
///
/// Lock order: the state of a registration, then TASKS, then NEW_TASKS, then
/// REGISTRATIONS.
struct Registration {
    key: EventKey,
    callback: CallbackEntry,
    state: Mutex<RegistrationState>,
    // signalled when a callback stops running
    stopped: Condvar,
}

impl Registration {
    fn new(key: EventKey, callback: CallbackEntry) -> Arc<Self> {
        remember_registration(key.clone(), CallbackEntry(callback.0, callback.1));
        Arc::new(Self {
            key,
            callback,
            state: Mutex::new(RegistrationState {
                phase: Phase::Queued,
                registered: true,
            }),
            stopped: Condvar::new(),
        })
    }

    fn forget(&self, state: &mut RegistrationState) {
        if std::mem::take(&mut state.registered) {
            forget_registration(&self.key, &self.callback);
        }
    }

    /// The task is about to run the callback, false if it was cancelled
    fn start(&self) -> bool {
        let mut state = self.state.lock().unwrap();
        if state.phase != Phase::Queued {
            return false;
        }
        state.phase = Phase::Running(std::thread::current().id());
        // the running callback re-registers its events, so this one doesn't count
        // as pending for register_unique
        self.forget(&mut state);
        true
    }

    /// The callback returned, returns whether it is queued again
    fn stop(&self, ready: bool) -> bool {
        let mut state = self.state.lock().unwrap();
        let queued = match state.phase {
            Phase::Running(_) if !ready => {
                state.phase = Phase::Queued;
                remember_registration(
                    self.key.clone(),
                    CallbackEntry(self.callback.0, self.callback.1),
                );
                state.registered = true;
                true
            }
            _ => {
                state.phase = Phase::Done;
                false
            }
        };
        drop(state);
        self.stopped.notify_all();
        queued
    }

    /// Queue the task again (unless it was cancelled meanwhile), the state is
    /// locked while doing so (see `cancel`)
    fn requeue(&self, task: QueuedEvent) {
        let state = self.state.lock().unwrap();
        if state.phase == Phase::Queued {
            TASKS.lock().unwrap().push_back(task);
            drop(state);
        } else {
            drop(state);
            drop(task);
        }
    }

    /// See `callback-registration.cancel`, returns the data if the caller owns it
    /// now, the task still has to be removed from the queue
    fn cancel(&self) -> Option<*mut OpaqueData> {
        let me = std::thread::current().id();
        let mut state = self.state.lock().unwrap();
        loop {
            match state.phase {
                Phase::Queued => {
                    state.phase = Phase::Done;
                    self.forget(&mut state);
                    return Some(self.callback.1);
                }
                Phase::Running(thread) | Phase::RunningCancelled(thread) if thread == me => {
                    // called from inside the callback (or a nested loop it runs),
                    // waiting would deadlock
                    state.phase = Phase::RunningCancelled(thread);
                    return None;
                }
                Phase::Running(_) | Phase::RunningCancelled(_) => {
                    state = self.stopped.wait(state).unwrap();
                }
                Phase::Done => return None,
            }
        }
    }

    fn is_pending(&self) -> bool {
        matches!(
            self.state.lock().unwrap().phase,
            Phase::Queued | Phase::Running(_)
        )
    }

    fn is_done(&self) -> bool {
        self.state.lock().unwrap().phase == Phase::Done
    }
}

/// The right to run the executor loop, held while running the tasks (the tasks
/// themselves are kept in `TASKS`).
struct Executor {
    change_event: Option<EventFd>,
}

thread_local! {
    // set while this thread runs callbacks of the executor
    static IN_TICK: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

// copy of Executor::change_event, readable while the executor is locked (by a callback
// calling block_on)
static CHANGE_EVENT: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(INVALID_FD);

/// The registered callbacks, except the ones which are running right now and the
/// ones registered while the executor runs callbacks (`NEW_TASKS`).
///
/// This lock is never held while running a callback, so a callback can register new
/// ones and run a nested loop (`block_on`, `cancel_task`) which processes the other
/// tasks, including the ones the outer loop has not reached yet.
static TASKS: Mutex<VecDeque<QueuedEvent>> = Mutex::new(VecDeque::new());
// while executing tasks from the loop new ones are collected here, they are added
// to TASKS by the next iteration of the loop (or a nested loop)
static EXECUTOR_BUSY: AtomicBool = AtomicBool::new(false);
static NEW_TASKS: Mutex<Vec<QueuedEvent>> = Mutex::new(Vec::new());

/// Add the tasks registered meanwhile to TASKS, returns whether there were any
fn collect_new_tasks() -> bool {
    // both locked, so that cancel finds the tasks in one of them
    let mut tasks = TASKS.lock().unwrap();
    let new_tasks = std::mem::take(&mut *NEW_TASKS.lock().unwrap());
    if new_tasks.is_empty() {
        false
    } else {
        if DEBUGGING {
            println!("{} Adding {} new tasks", gettid(), new_tasks.len());
        }
        tasks.extend(new_tasks);
        true
    }
}

// every pass over the tasks gets its own number, to visit each task once
static PASS_ID: AtomicU64 = AtomicU64::new(1);

impl Executor {
    fn change_event(&mut self) -> EventFd {
        *self.change_event.get_or_insert_with(|| {
            let fd = unsafe { libc::eventfd(0, libc::EFD_NONBLOCK) };
            if DEBUGGING {
                println!("{} change event fd={fd}", gettid());
            }
            CHANGE_EVENT.store(fd, Ordering::Release);
            fd
        })
    }

    /// run the executor until it would block, returns number of handled events
    fn tick(_ex: &mut std::sync::MutexGuard<'_, Self>, ws: &mut WaitSet) -> (usize, usize) {
        let old_busy = EXECUTOR_BUSY.swap(true, Ordering::Acquire);
        assert!(!old_busy);
        let result = Self::tick_tasks(ws);
        let old_busy = EXECUTOR_BUSY.swap(false, Ordering::Release);
        assert!(old_busy);
        result
    }

    /// Visit each task in TASKS once, running the callbacks of the ready ones. Returns the number of handled events and of
    /// waiting tasks.
    fn tick_tasks(ws: &mut WaitSet) -> (usize, usize) {
        let was_in_tick = IN_TICK.replace(true);
        let pass = PASS_ID.fetch_add(1, Ordering::Relaxed);
        let mut count_events = 0;
        let mut count_waiting = 0;
        let now = SystemTime::now();
        loop {
            let mut task = {
                let mut tasks = TASKS.lock().unwrap();
                match tasks.iter().position(|task| task.pass != pass) {
                    Some(pos) => tasks.remove(pos).unwrap(),
                    None => break,
                }
            };
            task.pass = pass;
            if task.reg.is_done() {
                // cancelled while in the hands of a tick
                drop(task);
                continue;
            }
            let run = if task.inner.ready() {
                if DEBUGGING {
                    println!(
                        "{} task ready {:x} {:x}",
                        gettid(),
                        task.reg.callback.0 as usize,
                        task.reg.callback.1 as usize
                    );
                }
                count_events += 1;
                true
            } else {
                count_waiting += 1;
                match &task.inner {
                    EventType::Triggered {
                        last_counter: _,
                        event: _,
                    } => {
                        ws.register(task.event_fd);
                        false
                    }
                    EventType::SystemTime(system_time) => {
                        if *system_time > now {
                            let diff = system_time.duration_since(now).unwrap_or_default();
                            ws.timeout(diff);
                            false
                        } else {
                            true
                        }
                    }
                }
            };
            if run && !task.run_callback() {
                // the task is dropped here, without holding the lock of TASKS
                drop(task);
                continue;
            }
            let reg = Arc::clone(&task.reg);
            reg.requeue(task);
        }
        IN_TICK.set(was_in_tick);
        (count_events, count_waiting)
    }
}

/// Make a thread blocked in the executor loop collect the tasks which were
/// registered meanwhile (by another thread). Nothing to do if the loop has
/// never run (it will see the tasks when it starts).
fn wake_executor() {
    let fd = CHANGE_EVENT.load(Ordering::Acquire);
    if fd != INVALID_FD {
        event_fd::activate(fd);
    }
}

static EXECUTOR: Mutex<Executor> = Mutex::new(Executor { change_event: None });

impl symmetric_executor::Guest for Guest {
    type CallbackFunction = Ignore;
    type CallbackData = OpaqueData;
    type CallbackRegistration = CallbackRegistrationInternal;
    type EventSubscription = EventSubscriptionInternal;
    type EventGenerator = EventGenerator;

    fn run() {
        run_until(None);
    }

    fn register(
        trigger: symmetric_executor::EventSubscription,
        callback: symmetric_executor::CallbackFunction,
        data: symmetric_executor::CallbackData,
    ) -> symmetric_executor::CallbackRegistration {
        let cb: CallbackType = unsafe { transmute(callback.take_handle()) };
        let data = data.take_handle() as *mut OpaqueData;

        // try to take a short cut
        let trigger: EventSubscriptionInternal = trigger.into_inner();
        if trigger.inner.ready() {
            if DEBUGGING {
                println!(
                    "{} register ready event {:x} {:x}",
                    gettid(),
                    cb as usize,
                    data as usize
                );
            }
            if matches!((cb)(data), CallbackState::Ready) {
                if DEBUGGING {
                    println!("{} registration unnecessary", gettid());
                }
                return symmetric_executor::CallbackRegistration::new(
                    CallbackRegistrationInternal(None),
                );
            }
        }

        let subscr = QueuedEvent::new(trigger, CallbackEntry(cb, data));
        let reg = Arc::clone(&subscr.reg);
        match EXECUTOR.try_lock() {
            Ok(mut lock) => {
                TASKS.lock().unwrap().push_back(subscr);
                let mut ws = WaitSet::new(None);
                // process as long as there is immediate progress
                loop {
                    let (count_events, _count_waiting) = Executor::tick(&mut lock, &mut ws);
                    if count_events == 0 {
                        break;
                    }
                }
                // wake other threads last
                event_fd::activate(lock.change_event());
            }
            Err(_err) => {
                if EXECUTOR_BUSY.load(Ordering::Acquire) {
                    NEW_TASKS.lock().unwrap().push(subscr);
                } else {
                    TASKS.lock().unwrap().push_back(subscr);
                }
                // The thread running the executor might just have decided to
                // wait for events (without this task), so wake it up.
                wake_executor();
            }
        }
        symmetric_executor::CallbackRegistration::new(CallbackRegistrationInternal(Some(reg)))
    }

    fn register_unique(
        trigger: symmetric_executor::EventSubscription,
        callback: symmetric_executor::CallbackFunction,
        data: symmetric_executor::CallbackData,
    ) -> Option<symmetric_executor::CallbackRegistration> {
        let cb: CallbackType = unsafe { transmute(callback.handle()) };
        let data2 = data.handle() as *mut OpaqueData;
        let key = EventKey::of(&trigger.get::<EventSubscriptionInternal>().inner);
        // pending registrations are tracked separately from the executor's task list, which can
        // be locked (or borrowed by tick, when called from a callback) right now
        if is_registered(&key, &CallbackEntry(cb, data2)) {
            _ = callback.take_handle();
            _ = data.take_handle();
            None
        } else {
            Some(Self::register(trigger, callback, data))
        }
    }

    fn block_on(trigger: symmetric_executor::EventSubscription) {
        let trigger: EventSubscriptionInternal = trigger.into_inner();
        if DEBUGGING {
            match &trigger.inner {
                EventType::Triggered {
                    last_counter: _,
                    event,
                } => println!(
                    "{} block_on(Trigger {:x})",
                    gettid(),
                    Arc::as_ptr(event) as usize
                ),
                EventType::SystemTime(system_time) => {
                    let diff = match system_time.duration_since(SystemTime::now()) {
                        Ok(diff) => format!("{}.{}", diff.as_secs(), diff.subsec_nanos()),
                        Err(err) => format!("{err}"),
                    };
                    println!("{} block_on(Time {})", gettid(), diff);
                }
            }
        }
        wait_for(trigger);
    }

    fn cancel_task(task: symmetric_executor::EventSubscription) -> bool {
        let task: EventSubscriptionInternal = task.into_inner();
        let (event, last_counter) = match &task.inner {
            EventType::Triggered {
                last_counter,
                event,
            } => (Arc::clone(event), last_counter.load(Ordering::Acquire)),
            // a timeout is no task, nothing to wait for
            EventType::SystemTime(_) => return false,
        };
        let cancel = {
            let inner = event.lock().unwrap();
            if inner.counter != last_counter {
                // completed already (or cancelled on its own)
                if DEBUGGING {
                    println!(
                        "{} cancel_task({:x}) completed already",
                        gettid(),
                        Arc::as_ptr(&event) as usize
                    );
                }
                return inner.cancelled;
            }
            inner.cancel.clone()
        };
        if DEBUGGING {
            println!(
                "{} cancel_task({:x}) cancel={:x?}",
                gettid(),
                Arc::as_ptr(&event) as usize,
                cancel.as_ref().map(|c| Arc::as_ptr(c) as usize)
            );
        }
        // no lock held here
        if let Some(cancel) = cancel {
            activate_event(&cancel, false);
        }
        wait_for(task);
        let cancelled = event.lock().unwrap().cancelled;
        cancelled
    }
}

/// `block_on` for a subscription
fn wait_for(trigger: EventSubscriptionInternal) {
    // part of this function is never used
    let queue = QueuedEvent::new(
        trigger,
        CallbackEntry(|_| CallbackState::Ready, std::ptr::null_mut()),
    );
    if queue.inner.ready() {
        if DEBUGGING {
            println!("{} block_on already ready", gettid());
        }
        // this can happen if the subscription is a bit older
        return;
    }
    run_until(Some(queue));
    if DEBUGGING {
        println!("{} block_on ENDED", gettid());
    }
}

/// `block_on` called from a callback, i.e. while this thread is already running the
/// executor (and holds its lock). All tasks except the running callbacks are
/// available here (they are in `TASKS`).
fn run_until_nested(event: Option<QueuedEvent>) {
    let change_event = CHANGE_EVENT.load(Ordering::Acquire);
    loop {
        let mut ws = WaitSet::new((change_event != INVALID_FD).then_some(change_event));
        if let Some(event) = event.as_ref() {
            if event.event_fd != INVALID_FD {
                ws.register(event.event_fd);
            }
        }
        collect_new_tasks();
        let (count_events, _count_waiting) = Executor::tick_tasks(&mut ws);
        if collect_new_tasks() {
            continue;
        }
        if TASKS.lock().unwrap().is_empty() {
            break;
        }
        if count_events != 0 {
            continue;
        }
        let selectresult = ws.wait();
        let mut exit = false;
        if selectresult > 0 {
            for i in ws.iter_active() {
                event_fd::consume(i);
                if event.as_ref().map_or(false, |event| i == event.event_fd) {
                    exit = true;
                }
            }
        }
        if exit {
            break;
        }
    }
}

fn run_until(event: Option<QueuedEvent>) {
    if IN_TICK.get() {
        return run_until_nested(event);
    }
    let change_event = EXECUTOR.lock().unwrap().change_event();
    loop {
        let mut ws = WaitSet::new(Some(change_event));
        if let Some(event) = event.as_ref() {
            if event.event_fd != INVALID_FD {
                ws.register(event.event_fd);
            }
        }
        let (count_events, count_waiting) = {
            let mut ex = EXECUTOR.lock().unwrap();
            let result = Executor::tick(&mut ex, &mut ws);
            if collect_new_tasks() {
                // collect callbacks and timeouts again
                continue;
            }
            result
        };
        if TASKS.lock().unwrap().is_empty() {
            if DEBUGGING {
                println!("{} No work left, exiting run_until()", gettid(),);
            }
            break;
        }
        if count_events != 0 {
            // we processed events, perhaps more became ready
            if DEBUGGING {
                println!(
                    "{} Relooping with {} tasks after {count_events} events, {count_waiting} waiting",
                    gettid(),
                    TASKS.lock().unwrap().len()
                );
            }
            continue;
        }
        // with no work left the break should have occured
        // assert!(!tvptr.is_null() || maxfd > 0);
        if DEBUGGING {
            ws.debug();
        }
        let selectresult = ws.wait();
        let mut exit = false;
        // we could look directly for the timeout
        if selectresult > 0 {
            // reset active file descriptors
            for i in ws.iter_active() {
                event_fd::consume(i);
                if event.as_ref().map_or(false, |event| i == event.event_fd) {
                    exit = true;
                    if DEBUGGING {
                        println!("{} Awaited signal active", gettid(),);
                    }
                }
            }
        }
        if exit {
            break;
        }
    }
}

type Count = u32;

struct EventInner {
    counter: Count,
    waiting: Vec<EventFd>,
    // callee side: activated when the task signalled by this event is cancelled
    cancel: Option<Arc<Mutex<EventInner>>>,
    // the task ended because of a cancellation (activate_cancelled)
    cancelled: bool,
}

struct EventGenerator(Arc<Mutex<EventInner>>);

type CallbackType = fn(*mut OpaqueData) -> CallbackState;

#[allow(unpredictable_function_pointer_comparisons)]
#[derive(PartialEq)]
struct CallbackEntry(CallbackType, *mut OpaqueData);

unsafe impl Send for CallbackEntry {}
unsafe impl Sync for CallbackEntry {}

struct EventSubscriptionInternal {
    inner: EventType,
}

struct QueuedEvent {
    inner: EventType,
    event_fd: EventFd,
    reg: Arc<Registration>,
    // the last pass of the executor which visited this task
    pass: u64,
}

/// Identity of the event a registration waits for
#[derive(Clone, PartialEq)]
enum EventKey {
    Trigger(usize),
    Time(SystemTime),
}

impl EventKey {
    fn of(inner: &EventType) -> Self {
        match inner {
            EventType::Triggered { event, .. } => Self::Trigger(Arc::as_ptr(event) as usize),
            EventType::SystemTime(time) => Self::Time(*time),
        }
    }
}

// The pending registrations, used by register_unique to avoid piling up identical ones
static REGISTRATIONS: Mutex<Vec<(EventKey, CallbackEntry)>> = Mutex::new(Vec::new());

fn remember_registration(key: EventKey, entry: CallbackEntry) {
    REGISTRATIONS.lock().unwrap().push((key, entry));
}

fn forget_registration(key: &EventKey, entry: &CallbackEntry) {
    let mut registrations = REGISTRATIONS.lock().unwrap();
    if let Some(pos) = registrations
        .iter()
        .position(|(k, e)| k == key && e == entry)
    {
        registrations.swap_remove(pos);
    }
}

fn is_registered(key: &EventKey, entry: &CallbackEntry) -> bool {
    REGISTRATIONS
        .lock()
        .unwrap()
        .iter()
        .any(|(k, e)| k == key && e == entry)
}

impl QueuedEvent {
    fn new(trigger: EventSubscriptionInternal, callback: CallbackEntry) -> Self {
        let inner = trigger.inner;
        let event_fd = match &inner {
            EventType::Triggered {
                last_counter: _,
                event,
            } => {
                let fd = unsafe { libc::eventfd(0, libc::EFD_NONBLOCK) };
                event.lock().unwrap().waiting.push(fd);
                fd
            }
            EventType::SystemTime(_system_time) => INVALID_FD,
        };
        if DEBUGGING {
            match &inner {
                EventType::Triggered {
                    last_counter: _,
                    event,
                } => println!(
                    "{} register(Trigger {:x} fd {event_fd}, {:x},{:x})",
                    gettid(),
                    Arc::as_ptr(event) as usize,
                    callback.0 as usize,
                    callback.1 as usize
                ),
                EventType::SystemTime(system_time) => {
                    let diff = match system_time.duration_since(SystemTime::now()) {
                        Ok(diff) => format!("{}.{}", diff.as_secs(), diff.subsec_nanos()),
                        Err(err) => format!("{err}"),
                    };
                    println!(
                        "{} register(Time {}, {:x},{:x})",
                        gettid(),
                        diff,
                        callback.0 as usize,
                        callback.1 as usize
                    );
                }
            }
        }
        QueuedEvent {
            reg: Registration::new(EventKey::of(&inner), callback),
            inner,
            event_fd,
            pass: 0,
        }
    }

    /// Run the callback (the event is ready), returns whether it stays registered
    /// (it returned pending and was not cancelled meanwhile).
    fn run_callback(&mut self) -> bool {
        if !self.reg.start() {
            return false;
        }
        let entry = &self.reg.callback;
        let result = (entry.0)(entry.1);
        if DEBUGGING {
            println!("{} callback returned {:?}", gettid(), result,);
        }
        self.reg.stop(matches!(result, CallbackState::Ready))
    }
}

impl EventSubscriptionInternal {
    fn dup(&self) -> Self {
        let inner = match &self.inner {
            EventType::Triggered {
                last_counter: last_counter_old,
                // event_fd,
                event,
            } => {
                let new_event = Arc::clone(event);
                let last_counter = last_counter_old.load(Ordering::Relaxed);
                if DEBUGGING {
                    println!(
                        "{} dup(subscr {last_counter} {:x})",
                        gettid(),
                        Arc::as_ptr(event) as usize
                    );
                }
                EventType::Triggered {
                    last_counter: AtomicU32::new(last_counter),
                    event: new_event,
                }
            }
            EventType::SystemTime(system_time) => EventType::SystemTime(*system_time),
        };
        EventSubscriptionInternal { inner }
    }
}

impl Drop for QueuedEvent {
    fn drop(&mut self) {
        {
            let mut state = self.reg.state.lock().unwrap();
            if DEBUGGING && state.phase != Phase::Done {
                println!(
                    "{} drop() with active callback {:x},{:x}",
                    gettid(),
                    self.reg.callback.0 as usize,
                    self.reg.callback.1 as usize
                );
            }
            state.phase = Phase::Done;
            self.reg.forget(&mut state);
        }
        match &self.inner {
            EventType::Triggered {
                last_counter: _,
                event,
            } => {
                if DEBUGGING {
                    println!("{} drop(queued fd {})", gettid(), self.event_fd);
                }
                event
                    .lock()
                    .unwrap()
                    .waiting
                    .retain(|&e| e != self.event_fd);
                unsafe { libc::close(self.event_fd) };
            }
            EventType::SystemTime(_system_time) => (),
        }
    }
}

enum EventType {
    Triggered {
        last_counter: AtomicU32,
        event: Arc<Mutex<EventInner>>,
    },
    SystemTime(SystemTime),
}

impl EventType {
    pub fn ready(&self) -> bool {
        match self {
            EventType::Triggered {
                last_counter,
                event,
            } => {
                let current_counter = event.lock().unwrap().counter;
                let active = current_counter != last_counter.load(Ordering::Acquire);
                if active {
                    if DEBUGGING {
                        println!(
                            "{} transitioned ready {:x} {}->{} {:x}",
                            gettid(),
                            Arc::as_ptr(event) as usize,
                            last_counter.load(Ordering::Relaxed),
                            current_counter,
                            self as *const Self as usize,
                        );
                    }
                    last_counter.store(current_counter, Ordering::Release);
                } else {
                    if DEBUGGING {
                        println!(
                            "{} waiting {:x} {:x} {}",
                            gettid(),
                            Arc::as_ptr(event) as usize,
                            self as *const Self as usize,
                            current_counter
                        );
                    }
                }
                active
            }
            EventType::SystemTime(system_time) => *system_time <= SystemTime::now(),
        }
    }
}
