//@ args = '--skip foo'

include!(env!("BINDINGS"));

struct Test;

export!(Test);

impl exports::exports::Guest for Test {
    fn bar() {}
}

#[cfg_attr(target_arch = "wasm32", unsafe(export_name = "exports#foo"))]
pub extern "C" fn foo() {}
