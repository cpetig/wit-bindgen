#[allow(non_snake_case)]
pub extern "C" fn fooX3AfooX2FresourcesX00create() -> i32 {
    unsafe { component_b::b_impl::fooX3AfooX2FresourcesX23create() }
}

#[allow(non_snake_case)]
pub extern "C" fn fooX3AfooX2FresourcesX00consume(a: u32) {
    // let obj0 = mesh::foo::foo::resources::R::new(a);
    unsafe { component_b::b_impl::fooX3AfooX2FresourcesX23consume(a as i32) }
}

#[allow(non_snake_case)]
pub extern "C" fn fooX3AfooX2FresourcesX00X5BmethodX5DrX2Eadd(a: u32, b: u32) {
    // let obj0 = mesh::foo::foo::resources::R::new(a);

    let handle = Box::into_raw(Box::new(a));

    unsafe {
        component_b::b_impl::fooX3AfooX2FresourcesX23X5BmethodX5DrX2Eadd(
            handle as *mut u8,
            b as i32,
        );
    }
}
