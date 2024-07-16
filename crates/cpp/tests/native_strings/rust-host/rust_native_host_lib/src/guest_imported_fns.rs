pub mod foo {
    pub mod foo {
        pub mod strings {

            pub fn a(a: String) {
                println!("{a}");
            }
            pub fn b() -> String {
                let s = String::from("hello B");
                s
            }

            pub fn c(a: String, b: String) -> String {
                println!("guest imported: {} | {}", a, b);
                let s = String::from("hello C");
                s
            }
        }
    }
}
