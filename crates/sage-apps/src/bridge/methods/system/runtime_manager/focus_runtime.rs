use async_trait::async_trait;

use crate::bridge::methods::{BridgeContext, BridgeMethod, BridgeTools};
use crate::bridge::methods::system::runtime_manager::{parse_runtime_target_params};
use crate::bridge::{failure, success, RustBridgeApprovalRequest, RustBridgeRequest, RustBridgeResponse};
use crate::bridge::capabilities::{SystemBridgeCapability};
use crate::bridge::methods::shared::BridgeMethodCapability;
use crate::runtime::focus_runtime;

#[derive(Debug, Clone, Copy)]
pub struct RuntimeManagerFocusRuntime;

#[async_trait]
impl BridgeMethod for RuntimeManagerFocusRuntime {
    fn capability(&self) -> BridgeMethodCapability {
        BridgeMethodCapability::system(SystemBridgeCapability::RuntimeManagerFocusRuntime)
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

        match focus_runtime(
            tools.app_handle,
            tools.host_state,
            &params.app_id
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
