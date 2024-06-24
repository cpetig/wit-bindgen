use rust_native_host_lib::the_world_native::exports::foo::foo::strings::{a, b, c};

fn main() {
    a("hello a".into());
    {
        let b = b();
        println!("{}", b);
    }

    let c1 = "hello C1";
    let c2 = "hello C2";
    let c = c(c1.into(), c2.into());
    println!("{}",c);
}
