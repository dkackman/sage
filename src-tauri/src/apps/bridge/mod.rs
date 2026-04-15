pub mod batch;
pub mod core;
pub mod fetch;
pub mod websocket;

pub fn bootstrap_fragments() -> Vec<&'static str> {
    vec![
        core::bootstrap_js(),
        fetch::bootstrap_js(),
        batch::bootstrap_js(),
        websocket::bootstrap_js(),
    ]
}
