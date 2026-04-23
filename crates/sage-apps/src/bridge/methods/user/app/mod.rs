pub mod events;
pub mod get_capabilities;
pub mod get_info;
pub mod lifecycle;
pub mod request_capability_grant;
pub mod request_network_whitelist_grant;

use std::path::PathBuf;

use tauri::Manager;

pub use events::{
    GrantedCapabilitiesChangeEvent, GrantedNetworkWhitelistChangeEvent,
};
pub use get_capabilities::SageGetCapabilities;
pub use get_info::AppGetInfo;
pub use lifecycle::{
    SageAppLifecycleReadyToStop, SageAppLifecycleSetBeforeStopListener,
};
pub use request_capability_grant::SageRequestCapabilityGrant;
pub use request_network_whitelist_grant::SageRequestNetworkWhitelistGrant;

use crate::bridge::methods::BridgeTools;
use crate::bridge::{failure, RustBridgeRequest, RustBridgeResponse};

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
