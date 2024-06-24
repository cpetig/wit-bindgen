use std::slice;

use crate::the_world_native::foo;




pub extern "C" fn fooX3AfooX2FstringsX00a(arg0: *mut u8, arg1: usize) {
    let len0 = arg1;
    let byte_slice: &[u8] = unsafe { slice::from_raw_parts(arg0, len0) };
    foo::foo::strings::A(std::str::from_utf8(byte_slice).unwrap().into());
}

extern "C" fn fooX3AfooX2FstringsX00b(arg0: &[u8]) {
    todo!()
    // let result0 = foo::foo::strings::B();
    // let &vec1 = result0;
    // auto ptr1 = vec1.data();
    // auto len1 = vec1.size();
    // *((size_t *)(arg0 + 8)) = len1;
    // *((uint8_t **)(arg0 + 0)) = ptr1;
}