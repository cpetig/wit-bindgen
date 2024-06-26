pub mod b;
pub mod b_impl;

use once_cell::sync::Lazy;
use std::sync::{Arc, Mutex};
use wasmtime::component::{Resource, ResourceTable};

#[derive(Debug)]
// Wrapper struct for the raw pointer
struct SafePtr {
    pub ptr: *mut u8,
}

// Implementing Send and Sync for SafePtr
unsafe impl Send for SafePtr {}
unsafe impl Sync for SafePtr {}

// Using PhantomData to indicate that SafePtr owns a raw pointer
impl SafePtr {
    fn new(ptr: *mut u8) -> Self {
        SafePtr { ptr }
    }
}

static GLOBAL_MAP: Lazy<Arc<Mutex<ResourceTable>>> =
    Lazy::new(|| Arc::new(Mutex::new(ResourceTable::new())));

fn store_resource(value: *mut u8) -> i32 {
    let mut map = GLOBAL_MAP.lock().unwrap();

    let v = map.push(Arc::new(Mutex::new(SafePtr::new(value)))).unwrap();

    v.rep() as i32
}

fn get_resource(key: i32) -> Option<Arc<Mutex<SafePtr>>> {
    let map = GLOBAL_MAP.lock().unwrap();
    let key = Resource::new_own(key as u32);
    let v: &Arc<Mutex<SafePtr>> = map.get(&key).unwrap();

    Some(v.to_owned())
}

fn remove_resource(key: i32) {
    let mut map = GLOBAL_MAP.lock().unwrap();
    let key: Resource<Arc<Mutex<SafePtr>>> = Resource::new_own(key as u32);
    map.delete(key).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn it_works() {
        let value = 5;
        let ptr = Box::into_raw(Box::new(value));
        let mut map = GLOBAL_MAP.lock().unwrap();
        let v = map
            .push(Arc::new(Mutex::new(SafePtr::new(ptr.cast()))))
            .unwrap();
        dbg!(&v.rep());

        let key = v.rep();

        let ret: &SafePtr = map.get(&Resource::new_own(key)).unwrap();
        dbg!(ret);
        assert_eq!(Box::new(value), unsafe { Box::from_raw(ret.ptr) });
    }
}
