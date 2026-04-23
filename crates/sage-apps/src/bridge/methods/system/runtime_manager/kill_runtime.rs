use async_trait::async_trait;

use crate::bridge::methods::system::runtime_manager::{
    RuntimeTargetParams, SystemKillRuntimeResult,
};
use crate::bridge::methods::{BridgeContext, BridgeMethod, BridgeTools};
use crate::bridge::{failure, success, RustBridgeRequest, RustBridgeResponse};
use crate::runtime::kill_runtime_internal;

#[derive(Debug, Clone, Copy)]
pub struct SystemKillRuntime;

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

        match kill_runtime_internal(
            tools.app_handle,
            tools.host_state,
            &params.app_id,
            "user_kill",
        )
            .await
        {
            Ok(result) => match serde_json::to_value::<SystemKillRuntimeResult>(result) {
                Ok(value) => success(&request.channel, &request.id, value),
                Err(err) => failure(
                    &request.channel,
                    &request.id,
                    "internal_error",
                    format!("failed to encode system.killRuntime result: {err}"),
                ),
            },
            Err(err) => failure(&request.channel, &request.id, "internal_error", err),
        }
    }
}
