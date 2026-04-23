pub mod focus_runtime;
pub mod hide_runtime;
pub mod kill_runtime;
pub mod list_runtimes;

pub use focus_runtime::SystemFocusRuntime;
pub use hide_runtime::SystemHideRuntime;
pub use kill_runtime::SystemKillRuntime;
pub use list_runtimes::SystemListRuntimes;

pub use crate::runtime::{RuntimeTargetParams, SystemKillRuntimeResult};

use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use specta::Type;

use crate::bridge::methods::BridgeTools;
use crate::bridge::{failure, RustBridgeRequest, RustBridgeResponse};
use crate::runtime::SageAppRuntimeRecord;

fn now_ms() -> Result<i64, String> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("system clock error: {err}"))?
        .as_millis() as i64)
}

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

async fn get_runtime_record_by_app_id(
    tools: &BridgeTools<'_>,
    app_id: &str,
) -> Result<SageAppRuntimeRecord, String> {
    let runtime_id = {
        let runtime_by_app_id = tools.host_state.runtime.runtime_by_app_id.lock().await;
        runtime_by_app_id.get(app_id).cloned()
    }
        .ok_or_else(|| format!("runtime not found for app id: {app_id}"))?;

    let record = {
        let by_runtime_id = tools.host_state.runtime.by_runtime_id.lock().await;
        by_runtime_id.get(&runtime_id).cloned()
    }
        .ok_or_else(|| format!("runtime record not found for runtime id: {runtime_id}"))?;

    Ok(record)
}

async fn write_runtime_record(
    tools: &BridgeTools<'_>,
    record: SageAppRuntimeRecord,
) -> Result<(), String> {
    let mut by_runtime_id = tools.host_state.runtime.by_runtime_id.lock().await;
    by_runtime_id.insert(record.runtime_id.clone(), record);
    Ok(())
}
