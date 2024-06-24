pub mod b;
pub mod b_impl;
//pub mod resource_table;

use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

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

// Global HashMap with SafePtr
static GLOBAL_MAP: Lazy<Arc<Mutex<HashMap<i32, Arc<Mutex<SafePtr>>>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));

fn store_resource(value: *mut u8) -> i32 {
    let mut map = GLOBAL_MAP.lock().unwrap();
    let k = map.len() + 1;
    map.insert(k as i32, Arc::new(Mutex::new(SafePtr::new(value))));
    k as i32
}

fn get_resource(key: i32) -> Option<Arc<Mutex<SafePtr>>> {
    let map = GLOBAL_MAP.lock().unwrap();
    map.get(&key).cloned()
}

fn remove_resource(key: i32) {
    let mut map = GLOBAL_MAP.lock().unwrap();
    map.remove(&key);
}
