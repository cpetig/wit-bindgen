include!(env!("BINDINGS"));

struct Component;

export!(Component);

impl crate::exports::test::cross::string_delay::Guest for Component {
    async fn forward(s: String) -> String {
        if s == "now" {
            return "directly returned".to_string();
        }
        crate::test::cross::wait::sleep(3).await;
        format!("after sleep: {s}")
    }
}
