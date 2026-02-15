//@ wasmtime-flags = '-Wcomponent-model-async'

include!(env!("BINDINGS"));

use crate::a::b::the_test::f;

struct Component;

export!(Component);

impl Guest for Component {
    async fn run() {
        let mut stream = f();
        let result = stream.next().await;
        assert_eq!(result, Some(String::from("Hello")));
        let result = stream.next().await;
        assert_eq!(result, Some(String::from("World!")));
        let result = stream.next().await;
        assert_eq!(result, Some(String::from("From")));
        let result = stream.next().await;
        assert_eq!(result, Some(String::from("a")));
        let result = stream.next().await;
        assert_eq!(result, Some(String::from("stream.")));
        let result = stream.next().await;
        assert_eq!(result, None);
    }
}
