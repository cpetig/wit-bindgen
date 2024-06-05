use std::alloc::Layout;

use crate::__export_foo_foo_strings_cabi;
use crate::the_world::exports::foo::foo::strings::Guest;
struct MyWorld;

impl Guest for MyWorld {
    fn a(x: String) {
        crate::the_world::foo::foo::strings::a(&x);
    }

    fn b() -> String {
        crate::the_world::foo::foo::strings::b()
    }

    fn c(a: String, b: String) -> String {
        crate::the_world::foo::foo::strings::c(&a, &b)
    }
}

__export_foo_foo_strings_cabi!(MyWorld with_types_in crate::the_world::exports::foo::foo::strings);

// the crate wit-bindgen-rt doesn't work on native
#[no_mangle]
pub unsafe extern "C" fn cabi_realloc(
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
        std::alloc::alloc(layout)
    } else {
        debug_assert_ne!(new_len, 0, "non-zero old_len requires non-zero new_len!");
        layout = Layout::from_size_align_unchecked(old_len, align);
        std::alloc::realloc(old_ptr, layout, new_len)
    };
    if ptr.is_null() {
        unreachable!();
    }
    return ptr;
}
