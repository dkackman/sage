use async_trait::async_trait;

use crate::bridge::methods::{BridgeContext, BridgeMethod, BridgeTools};
use crate::bridge::methods::system::runtime_manager::{parse_runtime_target_params};
use crate::bridge::{RustBridgeApprovalRequest, RustBridgeRequest, RustBridgeResponse};
use crate::bridge::capabilities::SystemBridgeCapability;
use crate::bridge::methods::shared::BridgeMethodCapability;
use crate::runtime::hide_runtime;

#[derive(Debug, Clone, Copy)]
pub struct RuntimeManagerHideRuntime;


#[async_trait]
impl BridgeMethod for RuntimeManagerHideRuntime {
    fn capability(&self) -> BridgeMethodCapability {
        BridgeMethodCapability::system(SystemBridgeCapability::RuntimeManagerHideRuntime)
    }

    fn approval_request(&self, _ctx: BridgeContext<'_>, _request: &RustBridgeRequest) -> Option<RustBridgeApprovalRequest> {
        None
    }

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

        match hide_runtime(
            tools.app_handle,
            tools.host_state,
            &params.app_id,
        )
            .await
        {
            Ok(record) => match serde_json::to_value(record) {
                Ok(value) => RustBridgeResponse::success(&request.channel, &request.id, value),
                Err(err) => RustBridgeResponse::error(
                    &request.channel,
                    &request.id,
                    "internal_error",
                    format!("failed to encode runtime record: {err}"),
                ),
            },
            Err(err) => RustBridgeResponse::error(&request.channel, &request.id, "internal_error", err),
        }
    }
}
