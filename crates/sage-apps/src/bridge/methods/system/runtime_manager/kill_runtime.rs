use async_trait::async_trait;

use crate::bridge::methods::system::runtime_manager::{
    get_runtime_record_by_app_id, parse_runtime_target_params,
};
use crate::bridge::methods::{BridgeContext, BridgeMethod, BridgeTools};
use crate::bridge::{failure, success, RustBridgeRequest, RustBridgeResponse};
use crate::runtime::close_runtime_internal_with_reason;

#[derive(Debug, Clone, Copy)]
pub struct SystemKillRuntime;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SystemKillRuntimeResult {
    pub ok: bool,
    pub app_id: String,
}

#[async_trait]
impl BridgeMethod for SystemKillRuntime {
    async fn handle(
        &self,
        _ctx: BridgeContext<'_>,
        tools: BridgeTools<'_>,
        request: &RustBridgeRequest,
    ) -> RustBridgeResponse {
        let params = match parse_runtime_target_params(request) {
            Ok(value) => value,
            Err(response) => return response,
        };

        let _record = match get_runtime_record_by_app_id(&tools, &params.app_id).await {
            Ok(record) => record,
            Err(err) => {
                return failure(&request.channel, &request.id, "not_found", err);
            }
        };

        if let Err(err) = close_runtime_internal_with_reason(
            tools.app_handle,
            tools.host_state,
            &params.app_id,
            "user_kill",
        )
            .await
        {
            return failure(&request.channel, &request.id, "internal_error", err);
        }

        let result = SystemKillRuntimeResult {
            ok: true,
            app_id: params.app_id,
        };

        match serde_json::to_value(result) {
            Ok(value) => success(&request.channel, &request.id, value),
            Err(err) => failure(
                &request.channel,
                &request.id,
                "internal_error",
                format!("failed to encode system.killRuntime result: {err}"),
            ),
        }
    }
}
