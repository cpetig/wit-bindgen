//@ wasmtime-flags = '-Wcomponent-model-async'

include!(env!("BINDINGS"));

use crate::test::cross::string_delay::forward;
use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

struct Component;

export!(Component);

impl Guest for Component {
    async fn run() {
        // Completes without suspending.
        assert_eq!(forward("now".into()).await, "directly returned");

        // The middle component awaits an import of the leaf component which
        // only completes after the event loop ran, so the first poll must
        // be pending.
        let mut f = pin!(forward("x".into()));
        let mut cx = Context::from_waker(Waker::noop());
        assert!(matches!(f.as_mut().poll(&mut cx), Poll::Pending));
        assert_eq!(f.await, "after sleep: x");
    }
}
