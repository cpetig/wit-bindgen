use std::{
    mem::transmute,
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering},
        Arc, Mutex,
    },
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

#[allow(dead_code)]
struct CallbackRegistrationInternal(usize);

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
        if let Ok(mut event) = self.0.lock() {
            event.counter += 1;
            if DEBUGGING {
                println!(
                    "{} activate({:x}) counter={}",
                    gettid(),
                    Arc::as_ptr(&self.0) as usize,
                    event.counter
                );
            }
            event.waiting.iter().for_each(|fd| {
                event_fd::activate(*fd);
            });
        } else if DEBUGGING {
            println!("{} activate failure", gettid());
        }
    }
}

impl GuestCallbackRegistration for CallbackRegistrationInternal {
    fn cancel(_obj: symmetric_executor::CallbackRegistration) -> symmetric_executor::CallbackData {
        todo!()
    }
}

struct Executor {
    active_tasks: Vec<QueuedEvent>,
    change_event: Option<EventFd>,
}

thread_local! {
    // set while this thread runs callbacks of the executor
    static IN_TICK: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

// copy of Executor::change_event, readable while the executor is locked (by a callback
// calling block_on)
static CHANGE_EVENT: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(INVALID_FD);

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
    fn tick(ex: &mut std::sync::MutexGuard<'_, Self>, ws: &mut WaitSet) -> (usize, usize) {
        let old_busy = EXECUTOR_BUSY.swap(true, Ordering::Acquire);
        assert!(!old_busy);
        let was_in_tick = IN_TICK.replace(true);
        let result = Self::tick_tasks(&mut ex.active_tasks, ws);
        IN_TICK.set(was_in_tick);
        let old_busy = EXECUTOR_BUSY.swap(false, Ordering::Release);
        assert!(old_busy);
        result
    }

    /// run the given tasks until they would block, returns number of handled events
    fn tick_tasks(tasks: &mut Vec<QueuedEvent>, ws: &mut WaitSet) -> (usize, usize) {
        let mut count_events = 0;
        let mut count_waiting = 0;
        let now = SystemTime::now();
        tasks.iter_mut().for_each(|task| {
            if task.inner.ready() {
                if DEBUGGING {
                    println!(
                        "{} task ready {:x} {:x}",
                        gettid(),
                        task.callback.as_ref().unwrap().0 as usize,
                        task.callback.as_ref().unwrap().1 as usize
                    );
                }
                count_events += 1;
                let key = EventKey::of(&task.inner);
                let registered = &mut task.registered;
                task.callback.take_if(|entry| {
                    // the running callback re-registers its events, so this one doesn't count
                    // as pending for register_unique
                    if std::mem::take(registered) {
                        forget_registration(&key, entry);
                    }
                    let result = (entry.0)(entry.1);
                    if DEBUGGING {
                        println!("{} callback returned {:?}", gettid(), result,);
                    }
                    let ready = matches!(result, CallbackState::Ready);
                    if !ready {
                        remember_registration(key.clone(), CallbackEntry(entry.0, entry.1));
                        *registered = true;
                    }
                    ready
                });
            } else {
                count_waiting += 1;
                match &task.inner {
                    EventType::Triggered {
                        last_counter: _,
                        event: _,
                    } => {
                        ws.register(task.event_fd);
                    }
                    EventType::SystemTime(system_time) => {
                        if *system_time > now {
                            let diff = system_time.duration_since(now).unwrap_or_default();
                            ws.timeout(diff);
                        } else {
                            let key = EventKey::of(&task.inner);
                            let registered = &mut task.registered;
                            task.callback.take_if(|entry| {
                                if std::mem::take(registered) {
                                    forget_registration(&key, entry);
                                }
                                let ready = matches!((entry.0)(entry.1), CallbackState::Ready);
                                if !ready {
                                    remember_registration(
                                        key.clone(),
                                        CallbackEntry(entry.0, entry.1),
                                    );
                                    *registered = true;
                                }
                                ready
                            });
                        }
                    }
                }
            }
        });
        tasks.retain(|task| task.callback.is_some());
        (count_events, count_waiting)
    }
}

static EXECUTOR: Mutex<Executor> = Mutex::new(Executor {
    active_tasks: Vec::new(),
    change_event: None,
});
// while executing tasks from the loop we can't directly queue new ones
static EXECUTOR_BUSY: AtomicBool = AtomicBool::new(false);
static NEW_TASKS: Mutex<Vec<QueuedEvent>> = Mutex::new(Vec::new());

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
                    CallbackRegistrationInternal(0),
                );
            }
        }

        let subscr = QueuedEvent::new(trigger, CallbackEntry(cb, data));
        let id = subscr.id;
        match EXECUTOR.try_lock() {
            Ok(mut lock) => {
                lock.active_tasks.push(subscr);
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
                    // actually this is unlikely, but give it a try
                    EXECUTOR.lock().unwrap().active_tasks.push(subscr);
                }
            }
        }
        symmetric_executor::CallbackRegistration::new(CallbackRegistrationInternal(id))
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
}

// TODO: Make this more efficient by polling the event?
/// `block_on` called from a callback, i.e. while this thread is already running the
/// executor (and holds its lock). Tasks which are being processed by the outer loop are
/// not available, only the tasks registered meanwhile (like the one `block_on` waits for)
/// run here.
fn run_until_nested(event: Option<QueuedEvent>) {
    let change_event = CHANGE_EVENT.load(Ordering::Acquire);
    let mut local: Vec<QueuedEvent> = Vec::new();
    loop {
        local.append(&mut NEW_TASKS.lock().unwrap());
        let mut ws = WaitSet::new((change_event != INVALID_FD).then_some(change_event));
        if let Some(event) = event.as_ref() {
            ws.register(event.event_fd);
        }
        let (count_events, _count_waiting) = Executor::tick_tasks(&mut local, &mut ws);
        if !NEW_TASKS.lock().unwrap().is_empty() {
            continue;
        }
        if local.is_empty() {
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
    // let the outer loop take care of the remaining tasks
    NEW_TASKS.lock().unwrap().append(&mut local);
}

fn run_until(event: Option<QueuedEvent>) {
    if IN_TICK.get() {
        return run_until_nested(event);
    }
    let change_event = EXECUTOR.lock().unwrap().change_event();
    loop {
        let mut ws = WaitSet::new(Some(change_event));
        if let Some(event) = event.as_ref() {
            ws.register(event.event_fd);
        }
        let (count_events, count_waiting) = {
            let mut ex = EXECUTOR.lock().unwrap();
            let (count_events, count_waiting) = Executor::tick(&mut ex, &mut ws);
            {
                let mut new_tasks = NEW_TASKS.lock().unwrap();
                if !new_tasks.is_empty() {
                    if DEBUGGING {
                        println!("{} Adding {} new tasks", gettid(), new_tasks.len());
                    }
                    ex.active_tasks.append(&mut new_tasks);
                    // collect callbacks and timeouts again
                    continue;
                }
            }
            if ex.active_tasks.is_empty() {
                if DEBUGGING {
                    println!("{} No work left, exiting run_until()", gettid(),);
                }
                break;
            }
            (count_events, count_waiting)
        };
        if count_events != 0 {
            // we processed events, perhaps more became ready
            if DEBUGGING {
                println!(
                        "{} Relooping with {} tasks after {count_events} events, {count_waiting} waiting",
                        gettid(),
                        EXECUTOR.lock().unwrap().active_tasks.len()
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
}

struct EventGenerator(Arc<Mutex<EventInner>>);

type CallbackType = fn(*mut OpaqueData) -> CallbackState;

#[allow(unpredictable_function_pointer_comparisons)]
#[derive(PartialEq)]
struct CallbackEntry(CallbackType, *mut OpaqueData);

unsafe impl Send for CallbackEntry {}

struct EventSubscriptionInternal {
    inner: EventType,
}

struct QueuedEvent {
    id: usize,
    inner: EventType,
    event_fd: EventFd,
    callback: Option<CallbackEntry>,
    // whether callback is listed in REGISTRATIONS
    registered: bool,
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

static ID_SOURCE: AtomicUsize = AtomicUsize::new(1);

impl QueuedEvent {
    fn new(trigger: EventSubscriptionInternal, callback: CallbackEntry) -> Self {
        let id = ID_SOURCE.fetch_add(1, Ordering::Relaxed);
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
        remember_registration(EventKey::of(&inner), CallbackEntry(callback.0, callback.1));
        QueuedEvent {
            id,
            inner,
            callback: Some(callback),
            event_fd,
            registered: true,
        }
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
        if let (true, Some(cb)) = (self.registered, &self.callback) {
            forget_registration(&EventKey::of(&self.inner), cb);
        }
        if let Some(cb) = &self.callback {
            if DEBUGGING {
                println!(
                    "{} drop() with active callback {:x},{:x}",
                    gettid(),
                    cb.0 as usize,
                    cb.1 as usize
                );
            }
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
