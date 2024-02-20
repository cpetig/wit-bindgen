#![no_std]

extern crate alloc;

use alloc::boxed::Box;
use core::fmt;
use core::marker;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicU32, Ordering::Relaxed};

#[cfg(feature = "macros")]
pub use wit_bindgen_rust_macro::*;

// Re-export `bitflags` so that we can reference it from macros.
#[doc(hidden)]
pub use bitflags;

#[doc(hidden)]
pub mod rt {
    use crate::alloc::string::String;
    use crate::alloc::vec::Vec;

    pub use crate::{Resource, RustResource, WasmResource};

    /// Provide a hook for generated export functions to run static
    /// constructors at most once. wit-bindgen-rust generates a call to this
    /// function at the start of all component export functions. Importantly,
    /// it is not called as part of `cabi_realloc`, which is a *core* export
    /// func, but may not execute ctors, because the environment ctor in
    /// wasi-libc (before rust 1.69.0) calls an import func, which is not
    /// permitted by the Component Model when inside realloc.
    ///
    /// We intend to remove this once rust 1.69.0 stabilizes.
    #[cfg(target_arch = "wasm32")]
    pub fn run_ctors_once() {
        static mut RUN: bool = false;
        unsafe {
            if !RUN {
                // This function is synthesized by `wasm-ld` to run all static
                // constructors. wasm-ld will either provide an implementation
                // of this symbol, or synthesize a wrapper around each
                // exported function to (unconditionally) run ctors. By using
                // this function, the linked module is opting into "manually"
                // running ctors.
                extern "C" {
                    fn __wasm_call_ctors();
                }
                __wasm_call_ctors();
                RUN = true;
            }
        }
    }

    use super::alloc::alloc::Layout;

    // Re-export things from liballoc for convenient use.
    pub use super::alloc::{alloc, boxed, string, vec};

    #[cfg(feature = "realloc")]
    #[no_mangle]
    unsafe extern "C" fn cabi_realloc(
        old_ptr: *mut u8,
        old_len: usize,
        align: usize,
        new_len: usize,
    ) -> *mut u8 {
        let layout;
        let ptr = if old_len == 0 {
            if new_len == 0 {
                return align as *mut u8;
            }
            layout = Layout::from_size_align_unchecked(new_len, align);
            alloc::alloc(layout)
        } else {
            debug_assert_ne!(new_len, 0, "non-zero old_len requires non-zero new_len!");
            layout = Layout::from_size_align_unchecked(old_len, align);
            alloc::realloc(old_ptr, layout, new_len)
        };
        if ptr.is_null() {
            // Print a nice message in debug mode, but in release mode don't
            // pull in so many dependencies related to printing so just emit an
            // `unreachable` instruction.
            if cfg!(debug_assertions) {
                alloc::handle_alloc_error(layout);
            } else {
                #[cfg(target_arch = "wasm32")]
                core::arch::wasm32::unreachable();
                #[cfg(not(target_arch = "wasm32"))]
                unreachable!();
            }
        }
        return ptr;
    }

    pub unsafe fn dealloc(ptr: i32, size: usize, align: usize) {
        if size == 0 {
            return;
        }
        let layout = Layout::from_size_align_unchecked(size, align);
        alloc::dealloc(ptr as *mut u8, layout);
    }

    macro_rules! as_traits {
        ($(($trait_:ident $func:ident $ty:ident <=> $($tys:ident)*))*) => ($(
            pub fn $func<T: $trait_>(t: T) -> $ty {
                t.$func()
            }

            pub trait $trait_ {
                fn $func(self) -> $ty;
            }

            impl<'a, T: Copy + $trait_> $trait_ for &'a T {
                fn $func(self) -> $ty{
                    (*self).$func()
                }
            }

            $(
                impl $trait_ for $tys {
                    #[inline]
                    fn $func(self) -> $ty {
                        self as $ty
                    }
                }
            )*

        )*)
    }

    as_traits! {
        (AsI64 as_i64 i64 <=> i64 u64)
        (AsI32 as_i32 i32 <=> i32 u32 i16 u16 i8 u8 char usize)
        (AsF32 as_f32 f32 <=> f32)
        (AsF64 as_f64 f64 <=> f64)
    }

    pub unsafe fn string_lift(bytes: Vec<u8>) -> String {
        if cfg!(debug_assertions) {
            String::from_utf8(bytes).unwrap()
        } else {
            String::from_utf8_unchecked(bytes)
        }
    }

    pub unsafe fn invalid_enum_discriminant<T>() -> T {
        if cfg!(debug_assertions) {
            panic!("invalid enum discriminant")
        } else {
            core::hint::unreachable_unchecked()
        }
    }

    pub unsafe fn char_lift(val: u32) -> char {
        if cfg!(debug_assertions) {
            core::char::from_u32(val).unwrap()
        } else {
            core::char::from_u32_unchecked(val)
        }
    }

    pub unsafe fn bool_lift(val: u8) -> bool {
        if cfg!(debug_assertions) {
            match val {
                0 => false,
                1 => true,
                _ => panic!("invalid bool discriminant"),
            }
        } else {
            core::mem::transmute::<u8, bool>(val)
        }
    }
}

type RawRep<T> = Option<T>;

/// A type which represents a component model resource, either imported or
/// exported into this component.
///
/// This is a low-level wrapper which handles the lifetime of the resource
/// (namely this has a destructor). The `T` provided defines the component model
/// intrinsics that this wrapper uses.
///
/// One of the chief purposes of this type is to provide `Deref` implementations
/// to access the underlying data when it is owned.
///
/// This type is primarily used in generated code for exported and imported
/// resources.
#[repr(transparent)]
pub struct Resource<T: WasmResource> {
    // NB: This would ideally be `u32` but it is not. The fact that this has
    // interior mutability is not exposed in the API of this type except for the
    // `take_handle` method which is supposed to in theory be private.
    //
    // This represents, almost all the time, a valid handle value. When it's
    // invalid it's stored as `u32::MAX`.
    handle: AtomicU32,
    _marker: marker::PhantomData<Box<T>>,
}

/// A trait which all wasm resources implement, namely providing the ability to
/// drop a resource.
///
/// This generally is implemented by generated code, not user-facing code.
pub unsafe trait WasmResource {
    /// Invokes the `[resource-drop]...` intrinsic.
    unsafe fn drop(handle: u32);
}

/// A trait which extends [`WasmResource`] used for Rust-defined resources, or
/// those exported from this component.
///
/// This generally is implemented by generated code, not user-facing code.
pub unsafe trait RustResource: WasmResource {
    /// Invokes the `[resource-new]...` intrinsic.
    unsafe fn new(rep: usize) -> u32;
    /// Invokes the `[resource-rep]...` intrinsic.
    unsafe fn rep(handle: u32) -> usize;
}

impl<T: WasmResource> Resource<T> {
    #[doc(hidden)]
    pub unsafe fn from_handle(handle: u32) -> Self {
        assert!(handle != u32::MAX);
        Self {
            handle: AtomicU32::new(handle),
            _marker: marker::PhantomData,
        }
    }

    /// Takes ownership of the handle owned by `resource`.
    ///
    /// Note that this ideally would be `into_handle` taking `Resource<T>` by
    /// ownership. The code generator does not enable that in all situations,
    /// unfortunately, so this is provided instead.
    ///
    /// Also note that `take_handle` is in theory only ever called on values
    /// owned by a generated function. For example a generated function might
    /// take `Resource<T>` as an argument but then call `take_handle` on a
    /// reference to that argument. In that sense the dynamic nature of
    /// `take_handle` should only be exposed internally to generated code, not
    /// to user code.
    #[doc(hidden)]
    pub fn take_handle(resource: &Resource<T>) -> u32 {
        resource.handle.swap(u32::MAX, Relaxed)
    }

    #[doc(hidden)]
    pub fn handle(resource: &Resource<T>) -> u32 {
        resource.handle.load(Relaxed)
    }

    /// Creates a new Rust-defined resource from the underlying representation
    /// `T`.
    ///
    /// This will move `T` onto the heap to create a single pointer to represent
    /// it which is then wrapped up in a component model resource.
    pub fn new(val: T) -> Resource<T>
    where
        T: RustResource,
    {
        let rep = Box::into_raw(Box::new(Some(val))) as usize;
        unsafe {
            let handle = T::new(rep);
            Resource::from_handle(handle)
        }
    }

    #[doc(hidden)]
    pub unsafe fn dtor(rep: usize)
    where
        T: RustResource,
    {
        let _ = Box::from_raw(rep as *mut RawRep<T>);
    }

    /// Takes back ownership of the object, dropping the resource handle.
    pub fn into_inner(resource: Self) -> T
    where
        T: RustResource,
    {
        unsafe {
            let rep = T::rep(resource.handle.load(Relaxed));
            RawRep::take(&mut *(rep as *mut RawRep<T>)).unwrap()
        }
    }

    #[doc(hidden)]
    pub unsafe fn lift_borrow<'a>(rep: usize) -> &'a T
    where
        T: RustResource,
    {
        RawRep::as_ref(&*(rep as *const RawRep<T>)).unwrap()
    }
}

impl<T: RustResource> Deref for Resource<T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe {
            let rep = T::rep(self.handle.load(Relaxed));
            RawRep::as_ref(&*(rep as *const RawRep<T>)).unwrap()
        }
    }
}

impl<T: RustResource> DerefMut for Resource<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe {
            let rep = T::rep(self.handle.load(Relaxed));
            RawRep::as_mut(&mut *(rep as *mut RawRep<T>)).unwrap()
        }
    }
}

impl<T: WasmResource> fmt::Debug for Resource<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Resource")
            .field("handle", &self.handle)
            .finish()
    }
}

impl<T: WasmResource> Drop for Resource<T> {
    fn drop(&mut self) {
        unsafe {
            match self.handle.load(Relaxed) {
                // If this handle was "taken" then don't do anything in the
                // destructor.
                u32::MAX => {}

                // ... but otherwise do actually destroy it with the imported
                // component model intrinsic as defined through `T`.
                other => T::drop(other),
            }
        }
    }
}
