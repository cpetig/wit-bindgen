use crate::b::exports::foo::foo::resources::Guest;
use crate::b::exports::foo::foo::resources::GuestR;
use crate::b::exports::foo::foo::resources::R;
use std::cell::RefCell;

#[derive(Debug)]
pub struct MyGuestR {
    val: RefCell<u32>,
}

impl GuestR for MyGuestR {
    fn new(a: u32) -> Self {
        MyGuestR {
            val: RefCell::new(a),
        }
    }

    fn add(&self, b: u32) {
        let mut val = self.val.borrow_mut();
        //dbg!(*val);
        *val += b;
        //println!("Note: add fails here because the variable in the trait is not mutable");
    }
}

struct MyGuest {}
impl Guest for MyGuest {
    type R = MyGuestR;
    fn create() -> R {
        let val = 18;
        println!("Created a resource with value: {}", val);
        let res = MyGuestR::new(val as u32);
        let res = R::new(res);
        // dbg!(&res);
        return res;
    }
    fn consume(o: R) {
        let p: &MyGuestR = o.get();
        println!("Consumed: {:?}", p.val);
    }
}

crate::__export_foo_foo_resources_cabi!(MyGuest with_types_in crate::b::exports::foo::foo::resources);

#[allow(non_snake_case)]
pub extern "C" fn X5BexportX5DfooX3AfooX2FresourcesX00X5Bresource_dropX5Dr(a: u32) {
    crate::remove_resource(a as i32);
}

type _RRep<T> = Option<T>;
#[allow(non_snake_case)]
pub extern "C" fn X5BexportX5DfooX3AfooX2FresourcesX00X5Bresource_newX5Dr(a: *mut u8) -> u32 {
    //dbg!(a);
    crate::store_resource(a) as u32
}

#[allow(non_snake_case)]
pub extern "C" fn X5BexportX5DfooX3AfooX2FresourcesX00X5Bresource_repX5Dr(id: i32) -> *mut u8 {
    let v = crate::get_resource(id)
        .unwrap()
        .to_owned()
        .lock()
        .unwrap()
        .ptr;

    v
}
