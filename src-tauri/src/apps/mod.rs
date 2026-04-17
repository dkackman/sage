pub mod builtin_apps;
pub mod csp;
pub mod install;
pub mod limits;
pub mod manifest;
pub mod package;
pub mod permission_registry;
pub mod permissions;
pub mod protocol;
pub mod registry;
pub mod sandbox;
pub mod snapshot;
pub mod storage;
pub mod types;
pub mod update;

pub use protocol::handle_app_protocol_request;
