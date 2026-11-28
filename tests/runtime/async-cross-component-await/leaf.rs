include!(env!("BINDINGS"));

use std::task::Poll;

struct Component;

export!(Component);

// Returns pending once after asking to be polled again, so the call only
// completes after the event loop ran.
async fn yield_once() {
    let mut yielded = false;
    std::future::poll_fn(|cx| {
        if yielded {
            Poll::Ready(())
        } else {
            yielded = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    })
    .await
}

impl crate::exports::test::cross::wait::Guest for Component {
    async fn sleep(rounds: u32) {
        for _ in 0..rounds {
            yield_once().await;
        }
    }
}
