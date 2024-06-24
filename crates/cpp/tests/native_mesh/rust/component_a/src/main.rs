mod a;
mod a_impl;

fn main() {
    let obj = a::foo::foo::resources::create();
    //dbg!("created object {:?}", &obj);
    // println!("{}", obj.handle());
    obj.add(5);
    a::foo::foo::resources::consume(obj);
}
