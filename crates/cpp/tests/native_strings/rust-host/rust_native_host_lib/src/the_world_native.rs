pub mod exports {
    pub mod foo {
        pub mod foo {
            pub mod strings {
                use core::slice;

                use crate::generated::{
                    fooX3AfooX2FstringsX23a, fooX3AfooX2FstringsX23b, fooX3AfooX2FstringsX23c,
                };
                pub fn a(a: String) {
                    unsafe {
                        let ptr0 = a.as_ptr().cast_mut();
                        let len0 = a.len();
                        fooX3AfooX2FstringsX23a(ptr0, len0);
                    };
                }

                pub fn b() -> &'static str {
                    unsafe {
                        let ret = fooX3AfooX2FstringsX23b();
                        let l1 = *ret.cast::<*const u8>();
                        let len = *ret.add(8) as usize;
                        let slice = slice::from_raw_parts(l1, len);
                        let d = match std::str::from_utf8(slice) {
                            Ok(utf8_str) => Ok(utf8_str), // Convert to String if successful
                            Err(e) => Err(e), // Return None if the byte slice is not valid UTF-8
                        };

                        return d.unwrap();
                    }
                }

                pub fn c(a: String, b: String) -> &'static str {
                    unsafe {
                        let ptr0 = a.as_ptr().cast_mut();
                        let len0 = a.len();
                        let ptr1 = b.as_ptr().cast_mut();
                        let len1 = b.len();
                        let ret = fooX3AfooX2FstringsX23c(ptr0, len0, ptr1, len1);
                        let l1 = *ret.cast::<*const u8>();
                        let len = *ret.add(8) as usize;
                        let slice = slice::from_raw_parts(l1, len);
                        let d = match std::str::from_utf8(slice) {
                            Ok(utf8_str) => Ok(utf8_str), // Convert to String if successful
                            Err(e) => Err(e), // Return None if the byte slice is not valid UTF-8
                        };

                        return d.unwrap();
                    }
                }
            }
        }
    }
}
