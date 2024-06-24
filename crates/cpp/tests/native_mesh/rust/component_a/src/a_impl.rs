use mesh::impl_mesh::fooX3AfooX2FresourcesX00X5BmethodX5DrX2Eadd as add;
use mesh::impl_mesh::fooX3AfooX2FresourcesX00consume as consume;
use mesh::impl_mesh::fooX3AfooX2FresourcesX00create as create;

#[allow(non_snake_case)]
pub extern "C" fn fooX3AfooX2FresourcesX00create() -> i32 {
    //component_b::b_impl::subtract(5, 6);
    create()
    //component_b::b_impl::fooX3AfooX2FresourcesX23create()
}

#[allow(non_snake_case)]
pub extern "C" fn fooX3AfooX2FresourcesX00consume(a: u32) {
    consume(a);
}

#[allow(non_snake_case)]
pub extern "C" fn fooX3AfooX2FresourcesX00X5Bresource_dropX5Dr(_a: u32) {
    dbg!("drop........");
}

#[allow(non_snake_case)]
pub extern "C" fn fooX3AfooX2FresourcesX00X5BconstructorX5Dr(a: i32) -> i32 {
    // Construct the resource here
    a
}
#[allow(non_snake_case)]
pub extern "C" fn fooX3AfooX2FresourcesX00X5BmethodX5DrX2Eadd(handle: i32, b: i32) {
    add(handle as u32, b as u32);
}
