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
    //dbg!(a);
    //let handle = Box::into_raw(Box::new(a));
    //get the actual object
    let handle =
        component_b::b_impl::X5BexportX5DfooX3AfooX2FresourcesX00X5Bresource_repX5Dr(a as i32);
    unsafe {
        component_b::b_impl::fooX3AfooX2FresourcesX23X5BmethodX5DrX2Eadd(handle, b as i32);
    }
}
