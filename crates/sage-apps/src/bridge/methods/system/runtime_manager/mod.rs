pub mod focus_runtime;
pub mod hide_runtime;
pub mod kill_runtime;
pub mod list_runtimes;
pub mod events;

pub use focus_runtime::RuntimeManagerFocusRuntime;
pub use hide_runtime::RuntimeManagerHideRuntime;
pub use kill_runtime::RuntimeManagerKillRuntime;
pub use list_runtimes::RuntimeManagerListRuntimes;
pub use events::RuntimeManagerRuntimesChangedEvent;

pub use crate::runtime::{RuntimeTargetParams, SystemKillRuntimeResult};
use crate::bridge::{failure, RustBridgeRequest, RustBridgeResponse};

fn parse_runtime_target_params(
    request: &RustBridgeRequest,
) -> Result<RuntimeTargetParams, RustBridgeResponse> {
    let Some(params_json) = request.params_json.clone() else {
        return Err(failure(
            &request.channel,
            &request.id,
            "invalid_request",
            "method requires params",
        ));
    };

    serde_json::from_str(&params_json).map_err(|err| {
        failure(
            &request.channel,
            &request.id,
            "invalid_request",
            format!("failed to decode params: {err}"),
        )
    })
}
