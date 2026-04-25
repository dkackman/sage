use async_trait::async_trait;

use crate::bridge::methods::system::runtime_manager::{parse_runtime_target_params};
use crate::bridge::methods::{BridgeContext, BridgeMethod, BridgeTools};
use crate::bridge::{failure, success, RustBridgeApprovalRequest, RustBridgeRequest, RustBridgeResponse};
use crate::bridge::capabilities::SystemBridgeCapability;
use crate::bridge::methods::shared::BridgeMethodCapability;
use crate::runtime::stop::{kill_runtime_internal, SystemKillRuntimeResult};

#[derive(Debug, Clone, Copy)]
pub struct RuntimeManagerKillRuntime;

#[async_trait]
impl BridgeMethod for RuntimeManagerKillRuntime {
    fn capability(&self) -> BridgeMethodCapability {
        BridgeMethodCapability::system(SystemBridgeCapability::RuntimeManagerKillRuntime)
    }

    fn approval_request(
        &self,
        _ctx: BridgeContext<'_>,
        _request: &RustBridgeRequest
    ) -> Option<RustBridgeApprovalRequest> {
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
