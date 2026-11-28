//! Tests of unregistering callbacks (`callback-registration.cancel` and
//! `is-pending` of `wit/executor.wit`): before and after the event fired, from
//! inside the callback, from another thread while the callback runs, together
//! with `register-unique`, and that no eventfd is left behind.
//!
//! The executor is global, so the cases run one after the other in a single test.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use wit_bindgen_symmetric_rt::async_support::{block_on, start_task, wait_on};
use wit_bindgen_symmetric_rt::{
    cancel_task, register, register_unique, run, unregister, CallbackRegistration,
    CallbackState, EventGenerator, EventSubscription,
};

fn open_fds() -> usize {
    std::fs::read_dir("/proc/self/fd").unwrap().count()
}

/// Let the executor process the events which are active (it runs until the
/// timeout, other registrations may remain)
fn process() {
    sleep(5);
}

/// Run the executor for a while
fn sleep(millis: u64) {
    block_on(wait_on(EventSubscription::from_timeout(millis * 1_000_000)));
}

#[derive(Default)]
struct Data {
    calls: AtomicU32,
    // returned by the callback
    ready: AtomicBool,
    // the callback cancels this registration of itself
    own: Mutex<Option<CallbackRegistration>>,
    own_result: Mutex<Option<bool>>,
    // the callback blocks for a while
    slow: AtomicBool,
    started: AtomicBool,
    finished: AtomicBool,
}

extern "C" fn callback(data: *mut Data) -> CallbackState {
    let data = unsafe { &*data };
    data.calls.fetch_add(1, Ordering::SeqCst);
    data.started.store(true, Ordering::SeqCst);
    if let Some(own) = data.own.lock().unwrap().take() {
        *data.own_result.lock().unwrap() = Some(unregister::<Data>(own).is_some());
    }
    if data.slow.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(100));
    }
    data.finished.store(true, Ordering::SeqCst);
    if data.ready.load(Ordering::SeqCst) {
        CallbackState::Ready
    } else {
        CallbackState::Pending
    }
}

fn ptr(data: &Data) -> *mut Data {
    data as *const Data as *mut Data
}

fn cancel_before_fire() {
    let data = Data::default();
    let generator = EventGenerator::new();
    let registration = register(generator.subscribe(), callback, ptr(&data));
    assert!(registration.is_pending());
    assert_eq!(unregister::<Data>(registration), Some(ptr(&data)));
    generator.activate();
    // nothing is registered any more, so this returns
    run();
    assert_eq!(data.calls.load(Ordering::SeqCst), 0);
}

fn cancel_after_fire() {
    // the callback returned ready
    let data = Data::default();
    data.ready.store(true, Ordering::SeqCst);
    let generator = EventGenerator::new();
    let registration = register(generator.subscribe(), callback, ptr(&data));
    generator.activate();
    run();
    assert_eq!(data.calls.load(Ordering::SeqCst), 1);
    assert!(!registration.is_pending());
    assert_eq!(unregister::<Data>(registration), None);

    // the callback returned pending: it would be called again
    let data = Data::default();
    let generator = EventGenerator::new();
    let registration = register(generator.subscribe(), callback, ptr(&data));
    generator.activate();
    process();
    assert_eq!(data.calls.load(Ordering::SeqCst), 1);
    assert!(registration.is_pending());
    assert_eq!(unregister::<Data>(registration), Some(ptr(&data)));
    generator.activate();
    run();
    assert_eq!(data.calls.load(Ordering::SeqCst), 1);

    // the event was active already, register called back directly
    let data = Data::default();
    data.ready.store(true, Ordering::SeqCst);
    let generator = EventGenerator::new();
    let subscription = generator.subscribe();
    generator.activate();
    let registration = register(subscription, callback, ptr(&data));
    assert_eq!(data.calls.load(Ordering::SeqCst), 1);
    assert!(!registration.is_pending());
    assert_eq!(unregister::<Data>(registration), None);
}

fn cancel_from_callback() {
    for ready in [false, true] {
        let data = Data::default();
        data.ready.store(ready, Ordering::SeqCst);
        let generator = EventGenerator::new();
        let registration = register(generator.subscribe(), callback, ptr(&data));
        *data.own.lock().unwrap() = Some(registration);
        generator.activate();
        // the callback is not called again (also not if it returns pending), so
        // run returns
        run();
        generator.activate();
        run();
        assert_eq!(data.calls.load(Ordering::SeqCst), 1);
        // running on this thread: the data stays with the callback
        assert_eq!(*data.own_result.lock().unwrap(), Some(false));
    }
}

fn cancel_from_other_thread() {
    for ready in [false, true] {
        let data = Arc::new(Data::default());
        data.slow.store(true, Ordering::SeqCst);
        data.ready.store(ready, Ordering::SeqCst);
        let generator = EventGenerator::new();
        let registration = register(generator.subscribe(), callback, ptr(&data));
        let generator2 = EventGenerator::new();
        let started = generator2.subscribe();
        let data2 = Arc::clone(&data);
        let canceller = std::thread::spawn(move || {
            while !data2.started.load(Ordering::SeqCst) {
                std::thread::yield_now();
            }
            assert!(!data2.finished.load(Ordering::SeqCst));
            let result = unregister::<Data>(registration);
            // waited for the callback to return
            assert!(data2.finished.load(Ordering::SeqCst));
            generator2.activate();
            result.map(|p| p as usize)
        });
        generator.activate();
        // runs the callback, until the canceller is done
        block_on(wait_on(started));
        let result = canceller.join().unwrap();
        if ready {
            assert_eq!(result, None);
        } else {
            assert_eq!(result, Some(ptr(&data) as usize));
        }
        generator.activate();
        run();
        assert_eq!(data.calls.load(Ordering::SeqCst), 1);
    }

    // cancelled while waiting (no callback running), the executor waits on
    // another thread
    let data = Arc::new(Data::default());
    let generator = EventGenerator::new();
    let registration = register(generator.subscribe(), callback, ptr(&data));
    let runner = std::thread::spawn(|| sleep(50));
    std::thread::sleep(Duration::from_millis(10));
    assert_eq!(unregister::<Data>(registration), Some(ptr(&data)));
    generator.activate();
    runner.join().unwrap();
    run();
    assert_eq!(data.calls.load(Ordering::SeqCst), 0);
}

fn register_unique_interaction() {
    let data = Data::default();
    let generator = EventGenerator::new();
    let first = register_unique(generator.subscribe(), callback, ptr(&data)).unwrap();
    // pending already
    assert!(register_unique(generator.subscribe(), callback, ptr(&data)).is_none());
    assert_eq!(unregister::<Data>(first), Some(ptr(&data)));
    // no longer pending, a new registration is made
    let second = register_unique(generator.subscribe(), callback, ptr(&data)).unwrap();
    assert!(register_unique(generator.subscribe(), callback, ptr(&data)).is_none());
    generator.activate();
    process();
    assert_eq!(data.calls.load(Ordering::SeqCst), 1);
    // returned pending, it stays registered
    assert!(register_unique(generator.subscribe(), callback, ptr(&data)).is_none());
    assert_eq!(unregister::<Data>(second), Some(ptr(&data)));
    let third = register_unique(generator.subscribe(), callback, ptr(&data)).unwrap();
    assert_eq!(unregister::<Data>(third), Some(ptr(&data)));
    run();
}

fn no_eventfd_leak() {
    let fds = open_fds();
    let data = Data::default();
    let generator = EventGenerator::new();
    let registrations: Vec<_> = (0..100)
        .map(|_| register(generator.subscribe(), callback, ptr(&data)))
        .collect();
    assert!(open_fds() >= fds + 100, "each registration has an eventfd");
    for registration in registrations {
        assert!(unregister::<Data>(registration).is_some());
    }
    assert_eq!(open_fds(), fds, "eventfds of cancelled registrations");
    run();
    assert_eq!(data.calls.load(Ordering::SeqCst), 0);
}

/// Pending until the generator (which is never activated) fires
fn task_waiting_forever(dropped: &Arc<AtomicBool>, generator: &EventGenerator) -> *mut u8 {
    struct DropFlag(Arc<AtomicBool>);
    impl Drop for DropFlag {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    let flag = DropFlag(Arc::clone(dropped));
    let wait = generator.subscribe();
    start_task(async move {
        let _flag = flag;
        wait_on(wait).await;
        unreachable!("never activated");
    })
}

fn cancelled_task_drops_registrations() {
    // the registrations of a cancelled task are gone right away, not when their
    // events fire
    let fds = open_fds();
    let dropped = Arc::new(AtomicBool::new(false));
    let never = EventGenerator::new();
    let task = task_waiting_forever(&dropped, &never);
    assert!(!task.is_null());
    let task = unsafe { EventSubscription::from_handle(task as usize) };
    assert!(cancel_task(task));
    assert!(dropped.load(Ordering::SeqCst));
    assert_eq!(open_fds(), fds, "registrations of the cancelled task");
    // and nothing keeps run() busy
    run();

    // the same from inside of a task
    let dropped = Arc::new(AtomicBool::new(false));
    let task = task_waiting_forever(&dropped, &never);
    let task = unsafe { EventSubscription::from_handle(task as usize) };
    let cancelled = block_on(async move { cancel_task(task) });
    assert!(cancelled);
    assert!(dropped.load(Ordering::SeqCst));
    run();
    assert_eq!(open_fds(), fds, "registrations of the cancelled task");
}

fn finished_task_drops_registrations() {
    // a task waiting for two events ends with the first one, the registration for
    // the other one is gone with it
    let fds = open_fds();
    let first = EventGenerator::new();
    let never = EventGenerator::new();
    let first_wait = first.subscribe();
    let never_wait = never.subscribe();
    let done = Arc::new(AtomicBool::new(false));
    let done2 = Arc::clone(&done);
    wit_bindgen_symmetric_rt::async_support::spawn(async move {
        let a = wait_on(first_wait);
        let b = wait_on(never_wait);
        futures::pin_mut!(a, b);
        futures::future::select(a, b).await;
        done2.store(true, Ordering::SeqCst);
    });
    first.activate();
    // returns although `never` is not activated
    run();
    assert!(done.load(Ordering::SeqCst));
    assert_eq!(open_fds(), fds, "registration for the other event");
}

#[test]
fn unregister_protocol() {
    // the executor keeps an eventfd once it ran
    run();
    let fds = open_fds();
    cancel_before_fire();
    cancel_after_fire();
    cancel_from_callback();
    cancel_from_other_thread();
    register_unique_interaction();
    no_eventfd_leak();
    cancelled_task_drops_registrations();
    finished_task_drops_registrations();
    run();
    assert_eq!(open_fds(), fds, "registrations left behind");
}
