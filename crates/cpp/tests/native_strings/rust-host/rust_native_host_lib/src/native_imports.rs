use crate::guest_imported_fns::foo::foo::strings::*;
use std::{ptr, slice};

#[allow(non_snake_case)]
pub extern "C" fn fooX3AfooX2FstringsX00a(arg0: *mut u8, arg1: usize) {
    unsafe {
        let slice = std::slice::from_raw_parts(arg0, arg1);
        let string_result = String::from_utf8(slice.to_vec());

        let ip = match string_result {
            Ok(string) => string,
            Err(_) => String::from("Invalid UTF-8 sequence"),
        };
        a(ip);
    }
}

#[allow(non_snake_case)]
pub extern "C" fn fooX3AfooX2FstringsX00b(arg0: *mut u8) {
    let result0 = b();

    unsafe {
        let len1 = result0.len();
        let ptr1 = result0.as_ptr();
        std::mem::forget(result0);
        let ptr_to_ptr1 = arg0 as *mut *const u8;
        ptr::write(ptr_to_ptr1, ptr1);

        // `arg0` + 8 offset to store the length (usize, which is 8 bytes on 64-bit systems)
        let ptr_to_len1 = arg0.add(8) as *mut usize;
        ptr::write(ptr_to_len1, len1);
    }
}

#[allow(non_snake_case)]
pub extern "C" fn fooX3AfooX2FstringsX00c(
    ptr0: *mut u8,
    len0: usize,
    ptr1: *mut u8,
    len1: usize,
    ptr2: *mut u8,
) {
    unsafe {
        let slice1 = slice::from_raw_parts(ptr0, len0);
        let str1 = match std::str::from_utf8(slice1) {
            Ok(utf8_str) => Ok(utf8_str), // Convert to String if successful
            Err(e) => Err(e),             // Return None if the byte slice is not valid UTF-8
        };

        let slice2 = slice::from_raw_parts(ptr1, len1);
        let str2 = match std::str::from_utf8(slice2) {
            Ok(utf8_str) => Ok(utf8_str), // Convert to String if successful
            Err(e) => Err(e),             // Return None if the byte slice is not valid UTF-8
        };
        let result0 = c(String::from(str1.unwrap()), String::from(str2.unwrap()));
        let len1 = result0.len();
        let ptr1 = result0.as_ptr();
        std::mem::forget(result0);
        let ptr_to_ptr1 = ptr2 as *mut *const u8;
        ptr::write(ptr_to_ptr1, ptr1);

        // `arg0` + 8 offset to store the length (usize, which is 8 bytes on 64-bit systems)
        let ptr_to_len1 = ptr2.add(8) as *mut usize;
        ptr::write(ptr_to_len1, len1);
    }
}
