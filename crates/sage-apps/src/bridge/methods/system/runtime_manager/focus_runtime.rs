use async_trait::async_trait;

use crate::bridge::methods::{BridgeContext, BridgeMethod, BridgeTools};
use crate::bridge::methods::system::runtime_manager::RuntimeTargetParams;
use crate::bridge::{failure, success, RustBridgeRequest, RustBridgeResponse};
use crate::runtime::focus_runtime_internal;

#[derive(Debug, Clone, Copy)]
pub struct SystemFocusRuntime;

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
impl BridgeMethod for SystemFocusRuntime {
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

        match focus_runtime_internal(
            tools.app_handle,
            tools.host_state,
            &params.app_id,
            true,
        )
            .await
        {
            Ok(record) => match serde_json::to_value(record) {
                Ok(value) => success(&request.channel, &request.id, value),
                Err(err) => failure(
                    &request.channel,
                    &request.id,
                    "internal_error",
                    format!("failed to encode runtime record: {err}"),
                ),
            },
            Err(err) => failure(&request.channel, &request.id, "internal_error", err),
        }
    }
}
