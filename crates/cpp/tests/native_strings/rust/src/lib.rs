use std::alloc::Layout;

use the_world::exports::foo::foo::strings::Guest;

use the_world::exports::foo::foo::strings;
#[macro_use]
mod the_world;
mod the_world_native;

struct MyWorld;

impl Guest for MyWorld {
    fn a(x: String,) {
        the_world::foo::foo::strings::a(&x);
    }

    fn b() -> String {
        the_world::foo::foo::strings::b()
    }

    fn c(a: String,b: String,) -> String {
        the_world::foo::foo::strings::c(&a, &b)
    }
}

the_world::export!(MyWorld with_types_in the_world);

__export_foo_foo_strings_cabi!(MyWorld with_types_in strings);


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
