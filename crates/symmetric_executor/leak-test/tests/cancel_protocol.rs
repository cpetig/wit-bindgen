//! Tests of the cancellation of tasks (`cancel-task`, `link-cancel` and
//! `activate-cancelled` of `wit/executor.wit`): callees with and without support
//! for cancellation, cancellation after completion and of tasks of async
//! exports, also while they wait for backpressure to be released.
//!
//! The executor is global, so the cases run one after the other in a single test.

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use wit_bindgen_symmetric_rt::async_support::{
    backpressure_dec, backpressure_inc, block_on, start_task,
};
use wit_bindgen_symmetric_rt::{
    cancel_task, register, CallbackState, EventGenerator, EventSubscription,
};

fn open_fds() -> usize {
    std::fs::read_dir("/proc/self/fd").unwrap().count()
}

/// Sets the flag when dropped
struct DropFlag(Arc<AtomicBool>);

impl Drop for DropFlag {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

/// Pending forever (no waker is kept)
struct Never;

impl Future for Never {
    type Output = ();
    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<()> {
        Poll::Pending
    }
}

/// An async export which never completes on its own, counting its polls
fn pending_export(dropped: &Arc<AtomicBool>, polls: &Arc<AtomicU32>) -> *mut u8 {
    let flag = DropFlag(Arc::clone(dropped));
    let polls = Arc::clone(polls);
    start_task(async move {
        let _flag = flag;
        polls.fetch_add(1, Ordering::SeqCst);
        Never.await;
        unreachable!("a cancelled task must not continue");
    })
}

fn subscription(handle: *mut u8) -> EventSubscription {
    assert!(!handle.is_null());
    unsafe { EventSubscription::from_handle(handle as usize) }
}

// The callbacks get the handle of a generator owned by the test (it is not
// consumed here).
extern "C" fn activate(generator: *mut u8) -> CallbackState {
    let generator = unsafe { EventGenerator::from_handle(generator as usize) };
    generator.activate();
    std::mem::forget(generator);
    CallbackState::Ready
}

extern "C" fn activate_cancelled(generator: *mut u8) -> CallbackState {
    let generator = unsafe { EventGenerator::from_handle(generator as usize) };
    generator.activate_cancelled();
    std::mem::forget(generator);
    CallbackState::Ready
}

fn handle(generator: &EventGenerator) -> *mut u8 {
    generator.handle() as *mut u8
}

fn cancel_after_completion() {
    let completion = EventGenerator::new();
    let task = completion.subscribe();
    completion.activate();
    assert!(!cancel_task(task), "completed normally");

    let completion = EventGenerator::new();
    let task = completion.subscribe();
    completion.activate_cancelled();
    assert!(cancel_task(task), "ended because of a cancellation");

    // a timeout is no task
    let task = EventSubscription::from_timeout(1_000_000_000);
    assert!(!cancel_task(task));
}

fn cancel_without_link_cancel() {
    // the callee doesn't support cancellation: the caller waits for it
    let completion = EventGenerator::new();
    let task = completion.subscribe();
    register(
        EventSubscription::from_timeout(20_000_000),
        activate,
        handle(&completion),
    );
    let start = std::time::Instant::now();
    assert!(!cancel_task(task), "the callee completed normally");
    assert!(start.elapsed() >= std::time::Duration::from_millis(20));
}

fn cancel_with_link_cancel() {
    // the callee stops when asked to
    let completion = EventGenerator::new();
    let cancel = EventGenerator::new();
    completion.link_cancel(&cancel);
    let task = completion.subscribe();
    register(cancel.subscribe(), activate_cancelled, handle(&completion));
    assert!(cancel_task(task), "the callee was cancelled");

    // the callee gets the request, but completes normally (results written)
    let completion = EventGenerator::new();
    let cancel = EventGenerator::new();
    completion.link_cancel(&cancel);
    let task = completion.subscribe();
    register(cancel.subscribe(), activate, handle(&completion));
    assert!(!cancel_task(task), "the callee completed");
}

fn cancel_export_task() {
    let dropped = Arc::new(AtomicBool::new(false));
    let polls = Arc::new(AtomicU32::new(0));
    let task = subscription(pending_export(&dropped, &polls));
    assert_eq!(polls.load(Ordering::SeqCst), 1);
    assert!(!dropped.load(Ordering::SeqCst));
    assert!(cancel_task(task), "the export was cancelled");
    assert!(dropped.load(Ordering::SeqCst), "the future is dropped");
    assert_eq!(polls.load(Ordering::SeqCst), 1);
}

fn cancel_completed_export_task() {
    let generator = EventGenerator::new();
    let wait = generator.subscribe();
    let task = subscription(start_task(async move {
        wit_bindgen_symmetric_rt::async_support::wait_on(wait).await;
    }));
    generator.activate();
    // the task completes while the executor runs
    wit_bindgen_symmetric_rt::run();
    assert!(!cancel_task(task), "the export completed");
}

fn cancel_queued_task() {
    let dropped = Arc::new(AtomicBool::new(false));
    let polls = Arc::new(AtomicU32::new(0));
    backpressure_inc();
    let task = subscription(pending_export(&dropped, &polls));
    assert!(!dropped.load(Ordering::SeqCst));
    assert!(cancel_task(task), "the queued task was cancelled");
    assert!(dropped.load(Ordering::SeqCst), "the future is dropped");
    backpressure_dec();
    assert_eq!(polls.load(Ordering::SeqCst), 0, "the task never started");
}

fn cancel_started_queued_task() {
    let dropped = Arc::new(AtomicBool::new(false));
    let polls = Arc::new(AtomicU32::new(0));
    backpressure_inc();
    let task = subscription(pending_export(&dropped, &polls));
    backpressure_dec();
    assert_eq!(polls.load(Ordering::SeqCst), 1, "started");
    assert!(cancel_task(task), "the started task was cancelled");
    assert!(dropped.load(Ordering::SeqCst), "the future is dropped");
    assert_eq!(polls.load(Ordering::SeqCst), 1);

    // a queued task which completes after it started
    backpressure_inc();
    let task = subscription(start_task(async {}));
    backpressure_dec();
    assert!(!cancel_task(task), "completed");
}

fn cancel_from_a_task() {
    // the caller is a task itself: cancel_task runs a nested executor loop,
    // which has to find the registration of the callee made before
    let dropped = Arc::new(AtomicBool::new(false));
    let polls = Arc::new(AtomicU32::new(0));
    let task = subscription(pending_export(&dropped, &polls));
    let generator = EventGenerator::new();
    let wait = generator.subscribe();
    let dropped2 = Arc::clone(&dropped);
    let cancelled = block_on(async move {
        // continue inside of the executor
        let wake = wit_bindgen_symmetric_rt::async_support::wait_on(wait);
        generator.activate();
        wake.await;
        assert!(!dropped2.load(Ordering::SeqCst));
        cancel_task(task)
    });
    assert!(cancelled);
    assert!(dropped.load(Ordering::SeqCst));
}

#[test]
fn cancel_task_protocol() {
    // the executor keeps an eventfd once it ran
    wit_bindgen_symmetric_rt::run();
    let fds = open_fds();
    cancel_after_completion();
    cancel_without_link_cancel();
    cancel_with_link_cancel();
    cancel_export_task();
    cancel_completed_export_task();
    cancel_queued_task();
    cancel_started_queued_task();
    cancel_from_a_task();
    // no registration is left behind (each one keeps an eventfd)
    wit_bindgen_symmetric_rt::run();
    assert_eq!(open_fds(), fds, "registrations left behind");
}
