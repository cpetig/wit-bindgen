// Generated by `wit-bindgen` 0.42.1. DO NOT EDIT!
// Options used:
#[rustfmt::skip]
#[allow(dead_code, clippy::all)]
pub mod exports {
    pub mod a {
        pub mod b {
            #[allow(dead_code, async_fn_in_trait, unused_imports, clippy::all)]
            pub mod the_test {
                #[used]
                #[doc(hidden)]
                static __FORCE_SECTION_REF: fn() = super::super::super::super::__link_custom_section_describing_imports;
                use super::super::super::super::_rt;
                #[doc(hidden)]
                #[allow(non_snake_case, unused_unsafe)]
                pub unsafe fn _export_f_cabi<T: Guest>() -> *mut u8 {
                    unsafe {
                        #[cfg(target_arch = "wasm32")] _rt::run_ctors_once();
                        let result0 = { T::f() };
                        (result0).take_handle() as *mut u8
                    }
                }
                pub trait Guest {
                    #[allow(async_fn_in_trait)]
                    fn f() -> wit_bindgen::rt::async_support::FutureReader<_rt::String>;
                }
                #[doc(hidden)]
                macro_rules! __export_a_b_the_test_cabi {
                    ($ty:ident with_types_in $($path_to_types:tt)*) => {
                        const _ : () = { #[cfg_attr(target_arch = "wasm32", export_name =
                        "f")] #[cfg_attr(not(target_arch = "wasm32"), no_mangle)]
                        #[allow(non_snake_case)] unsafe extern "C" fn
                        aX3AbX2Fthe_testX00f() -> * mut u8 { unsafe {
                        $($path_to_types)*:: _export_f_cabi::<$ty > () } } };
                    };
                }
                #[doc(hidden)]
                pub(crate) use __export_a_b_the_test_cabi;
            }
        }
    }
}
#[rustfmt::skip]
mod _rt {
    #![allow(dead_code, clippy::all)]
    pub use alloc_crate::string::String;
    pub unsafe fn cabi_dealloc(ptr: *mut u8, size: usize, align: usize) {
        if size == 0 {
            return;
        }
        unsafe {
            let layout = alloc::Layout::from_size_align_unchecked(size, align);
            alloc::dealloc(ptr, layout);
        }
    }
    #[cfg(target_arch = "wasm32")]
    pub fn run_ctors_once() {
        wit_bindgen::rt::run_ctors_once();
    }
    extern crate alloc as alloc_crate;
    pub use alloc_crate::alloc;
}
pub mod wit_future {
    #![allow(dead_code, unused_variables, clippy::all)]
    #[doc(hidden)]
    pub trait FuturePayload: Unpin + Sized + 'static {
        const VTABLE: &'static wit_bindgen::rt::async_support::FutureVtable<Self>;
    }
    #[doc(hidden)]
    #[allow(unused_unsafe)]
    pub mod vtable0 {
        unsafe fn lift(ptr: *mut u8) -> super::super::_rt::String {
            unsafe {
                let l0 = *ptr.add(0).cast::<*mut u8>();
                let l1 = *ptr.add(::core::mem::size_of::<*const u8>()).cast::<usize>();
                let len2 = l1;
                let string2 = String::from(
                    std::str::from_utf8(std::slice::from_raw_parts(l0, len2)).unwrap(),
                );
                string2
            }
        }
        unsafe fn lower(value: super::super::_rt::String, ptr: *mut u8) {
            unsafe {
                let vec0 = (value.into_bytes()).into_boxed_slice();
                let ptr0 = vec0.as_ptr().cast::<u8>();
                let len0 = vec0.len();
                ::core::mem::forget(vec0);
                *ptr.add(::core::mem::size_of::<*const u8>()).cast::<usize>() = len0;
                *ptr.add(0).cast::<*mut u8>() = ptr0.cast_mut();
            }
        }
        unsafe fn dealloc_lists(ptr: *mut u8) {
            unsafe {
                let l0 = *ptr.add(0).cast::<*mut u8>();
                let l1 = *ptr.add(::core::mem::size_of::<*const u8>()).cast::<usize>();
                super::super::_rt::cabi_dealloc(l0, l1, 1);
            }
        }
        pub static VTABLE: wit_bindgen::rt::async_support::FutureVtable<
            super::super::_rt::String,
        > = wit_bindgen::rt::async_support::FutureVtable::<super::super::_rt::String> {
            layout: unsafe {
                ::std::alloc::Layout::from_size_align_unchecked(
                    2 * ::core::mem::size_of::<*const u8>(),
                    ::core::mem::size_of::<*const u8>(),
                )
            },
            lift,
            lower,
        };
        impl super::FuturePayload for super::super::_rt::String {
            const VTABLE: &'static wit_bindgen::rt::async_support::FutureVtable<Self> = &VTABLE;
        }
    }
    /// Creates a new Component Model `future` with the specified payload type.
    ///
    /// The `default` function provided computes the default value to be sent in
    /// this future if no other value was otherwise sent.
    pub fn new<T: FuturePayload>(
        default: fn() -> T,
    ) -> (
        wit_bindgen::rt::async_support::FutureWriter<T>,
        wit_bindgen::rt::async_support::FutureReader<T>,
    ) {
        unsafe { wit_bindgen::rt::async_support::future_new::<T>(default, T::VTABLE) }
    }
}
/// Generates `#[unsafe(no_mangle)]` functions to export the specified type as
/// the root implementation of all generated traits.
///
/// For more information see the documentation of `wit_bindgen::generate!`.
///
/// ```rust
/// # macro_rules! export{ ($($t:tt)*) => (); }
/// # trait Guest {}
/// struct MyType;
///
/// impl Guest for MyType {
///     // ...
/// }
///
/// export!(MyType);
/// ```
#[allow(unused_macros)]
#[doc(hidden)]
macro_rules! __export_test_impl {
    ($ty:ident) => {
        self::export!($ty with_types_in self);
    };
    ($ty:ident with_types_in $($path_to_types_root:tt)*) => {
        $($path_to_types_root)*::
        exports::a::b::the_test::__export_a_b_the_test_cabi!($ty with_types_in
        $($path_to_types_root)*:: exports::a::b::the_test);
    };
}
#[doc(inline)]
pub(crate) use __export_test_impl as export;
#[rustfmt::skip]
#[cfg(target_arch = "wasm32")]
#[unsafe(link_section = "component-type:wit-bindgen:0.42.1:a:b:test:encoded world")]
#[doc(hidden)]
#[allow(clippy::octal_escapes)]
pub static __WIT_BINDGEN_COMPONENT_TYPE: [u8; 176] = *b"\
\0asm\x0d\0\x01\0\0\x19\x16wit-component-encoding\x04\0\x076\x01A\x02\x01A\x02\x01\
B\x03\x01e\x01s\x01@\0\0\0\x04\0\x01f\x01\x01\x04\0\x0ca:b/the-test\x05\0\x04\0\x08\
a:b/test\x04\0\x0b\x0a\x01\0\x04test\x03\0\0\0G\x09producers\x01\x0cprocessed-by\
\x02\x0dwit-component\x070.235.0\x10wit-bindgen-rust\x060.42.1";
#[inline(never)]
#[doc(hidden)]
pub fn __link_custom_section_describing_imports() {
    wit_bindgen::rt::maybe_link_cabi_realloc();
}
