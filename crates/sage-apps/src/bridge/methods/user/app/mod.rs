pub mod events;
pub mod get_capabilities;
pub mod get_info;
pub mod lifecycle;
pub mod request_capability_grant;
pub mod request_network_whitelist_grant;

use std::path::PathBuf;
use serde::Serialize;
use tauri::Manager;

pub use events::{
    GrantedCapabilitiesChangeEvent, GrantedNetworkWhitelistChangeEvent,
};
pub use get_capabilities::AppGetCapabilities;
pub use get_info::AppGetInfo;
pub use lifecycle::{
    AppLifecycleReadyToStop, AppLifecycleSetBeforeStopListener,
};
pub use request_capability_grant::AppRequestCapabilityGrant;
pub use request_network_whitelist_grant::AppRequestNetworkWhitelistGrant;

use crate::bridge::methods::BridgeTools;
use crate::bridge::{failure, success, RustBridgeRequest, RustBridgeResponse};

pub fn resolve_app_base_path(
    tools: &BridgeTools<'_>,
    request: &RustBridgeRequest,
) -> Result<PathBuf, RustBridgeResponse> {
    tools
        .app_handle
        .path()
        .app_data_dir()
        .map_err(|err| {
            failure(
                &request.channel,
                &request.id,
                "internal_error",
                format!("failed to resolve app data dir: {err}"),
            )
        })
}

pub fn encode_request_success<T: Serialize>(
    request: &RustBridgeRequest,
    value: T,
    context: &str,
) -> RustBridgeResponse {
    match serde_json::to_value(value) {
        Ok(value) => success(&request.channel, &request.id, value),
        Err(err) => failure(
            &request.channel,
            &request.id,
            "internal_error",
            format!("failed to encode {context}: {err}"),
        ),
    }
}
