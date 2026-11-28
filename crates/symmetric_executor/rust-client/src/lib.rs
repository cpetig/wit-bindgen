use core::ptr::{self, NonNull};
use executor_import::symmetric::runtime::symmetric_executor::{
    self, CallbackData, CallbackFunction,
};
pub use executor_import::symmetric::runtime::symmetric_executor::{
    run, CallbackState, EventGenerator, EventSubscription,
};
use std::alloc::{self, Layout};
pub use stream_import::symmetric::runtime::symmetric_stream;

pub mod async_support;
mod executor_import;
mod stream_import;

// Re-export `bitflags` so that we can reference it from macros.
#[cfg(feature = "bitflags")]
#[doc(hidden)]
pub use bitflags;

pub struct EventSubscription2;
pub struct EventGenerator2;

pub fn register<T>(
    event: EventSubscription,
    f: extern "C" fn(*mut T) -> CallbackState,
    data: *mut T,
) -> symmetric_executor::CallbackRegistration {
    let callback = unsafe { CallbackFunction::from_handle(f as *const () as usize) };
    let cb_data = unsafe { CallbackData::from_handle(data as usize) };
    symmetric_executor::register(event, callback, cb_data)
}

pub fn register_unique<T>(
    event: EventSubscription,
    f: extern "C" fn(*mut T) -> CallbackState,
    data: *mut T,
) -> Option<symmetric_executor::CallbackRegistration> {
    let callback = unsafe { CallbackFunction::from_handle(f as *const () as usize) };
    let cb_data = unsafe { CallbackData::from_handle(data as usize) };
    symmetric_executor::register_unique(event, callback, cb_data)
}

// #[no_mangle]
// fn cabi_realloc_wit_bindgen_0_41_0(
//     _old_ptr: *mut u8,
//     _old_len: usize,
//     _align: usize,
//     _new_len: usize,
// ) -> *mut u8 {
//     todo!()
// }

pub unsafe fn subscribe_event_send_ptr(event_send: *mut EventGenerator2) -> EventSubscription {
    let gener: EventGenerator = unsafe { EventGenerator::from_handle(event_send as usize) };
    // (unsafe {Arc::from_raw(event_send.cast()) });
    let subscription = gener.subscribe();
    // avoid consuming the generator
    std::mem::forget(gener);
    subscription
}

pub unsafe fn activate_event_send_ptr(event_send: *mut EventGenerator2) {
    let gener: EventGenerator = unsafe { EventGenerator::from_handle(event_send as usize) };
    gener.activate();
    // avoid consuming the generator
    std::mem::forget(gener);
}

// wait for an event subscription (e.g. returned from an async function)
pub unsafe fn wait_for_event(event_subscr: *mut EventSubscription2) {
    executor_import::symmetric::runtime::symmetric_executor::block_on(
        EventSubscription::from_handle(event_subscr as usize),
    );
}

// stolen from guest-rust/rt/src/lib.rs
pub struct Cleanup {
    ptr: NonNull<u8>,
    layout: Layout,
}

// Usage of the returned pointer is always unsafe and must abide by these
// conventions, but this structure itself has no inherent reason to not be
// send/sync.
unsafe impl Send for Cleanup {}
unsafe impl Sync for Cleanup {}

impl Cleanup {
    pub fn new(layout: Layout) -> (*mut u8, Option<Cleanup>) {
        if layout.size() == 0 {
            return (ptr::null_mut(), None);
        }
        let ptr = unsafe { alloc::alloc(layout) };
        let ptr = match NonNull::new(ptr) {
            Some(ptr) => ptr,
            None => alloc::handle_alloc_error(layout),
        };
        (ptr.as_ptr(), Some(Cleanup { ptr, layout }))
    }
    pub fn forget(self) {
        core::mem::forget(self);
    }
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        unsafe {
            alloc::dealloc(self.ptr.as_ptr(), self.layout);
        }
    }
}

/// Lowering of the parameters of an imported function in the symmetric ABI
/// passes borrowed pointers, the values must stay alive until the callee is
/// done with them. This hands owned values over to `deallocate` (the generated
/// code frees them after the call), borrowed values just stay where they are.
pub trait KeepAlive {
    fn keep_alive(self, deallocate: &mut Vec<(*mut u8, Layout)>) -> (*const u8, usize);
}

fn leak_boxed<T>(boxed: Box<[T]>, deallocate: &mut Vec<(*mut u8, Layout)>) -> (*const u8, usize) {
    let len = boxed.len();
    let layout = Layout::for_value(&*boxed);
    let ptr = Box::into_raw(boxed).cast::<u8>();
    if layout.size() != 0 {
        deallocate.push((ptr, layout));
    }
    (ptr, len)
}

impl KeepAlive for String {
    fn keep_alive(self, deallocate: &mut Vec<(*mut u8, Layout)>) -> (*const u8, usize) {
        leak_boxed(self.into_bytes().into_boxed_slice(), deallocate)
    }
}

impl<T> KeepAlive for Vec<T> {
    fn keep_alive(self, deallocate: &mut Vec<(*mut u8, Layout)>) -> (*const u8, usize) {
        leak_boxed(self.into_boxed_slice(), deallocate)
    }
}

impl<T> KeepAlive for Box<[T]> {
    fn keep_alive(self, deallocate: &mut Vec<(*mut u8, Layout)>) -> (*const u8, usize) {
        leak_boxed(self, deallocate)
    }
}

/// Values which stay where they are while borrowed.
pub trait Borrowed {
    fn ptr_len(&self) -> (*const u8, usize);
}

impl Borrowed for str {
    fn ptr_len(&self) -> (*const u8, usize) {
        (self.as_ptr(), self.len())
    }
}

impl Borrowed for String {
    fn ptr_len(&self) -> (*const u8, usize) {
        (self.as_ptr(), self.len())
    }
}

impl<T> Borrowed for [T] {
    fn ptr_len(&self) -> (*const u8, usize) {
        (self.as_ptr().cast(), self.len())
    }
}

impl<T> Borrowed for Vec<T> {
    fn ptr_len(&self) -> (*const u8, usize) {
        (self.as_ptr().cast(), self.len())
    }
}

impl<T: Borrowed + ?Sized> Borrowed for &T {
    fn ptr_len(&self) -> (*const u8, usize) {
        (**self).ptr_len()
    }
}

impl<T: Borrowed + ?Sized> KeepAlive for &T {
    fn keep_alive(self, _deallocate: &mut Vec<(*mut u8, Layout)>) -> (*const u8, usize) {
        self.ptr_len()
    }
}

pub fn keep_alive<T: KeepAlive>(
    value: T,
    deallocate: &mut Vec<(*mut u8, Layout)>,
) -> (*const u8, usize) {
    value.keep_alive(deallocate)
}
