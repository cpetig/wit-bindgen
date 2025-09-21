// this crate tries to minimize dependencies introduced by generated
// bindings

pub mod rt {
    pub fn maybe_link_cabi_realloc() {}

    use std::{alloc::{self, Layout}, ptr::{self, NonNull}};

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
}
