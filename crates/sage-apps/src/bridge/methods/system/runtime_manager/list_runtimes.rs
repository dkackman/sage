use async_trait::async_trait;

use crate::bridge::methods::{BridgeContext, BridgeMethod, BridgeTools};
use crate::bridge::{failure, success, RustBridgeApprovalRequest, RustBridgeRequest, RustBridgeResponse};
use crate::bridge::capabilities::SystemBridgeCapability;
use crate::bridge::methods::shared::BridgeMethodCapability;
use crate::runtime::state::read::list_runtimes;

#[derive(Debug, Clone, Copy)]
pub struct RuntimeManagerListRuntimes;

#[async_trait]
impl BridgeMethod for RuntimeManagerListRuntimes {
    fn capability(&self) -> BridgeMethodCapability {
        BridgeMethodCapability::system(SystemBridgeCapability::RuntimeManagerListRuntimes)
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
        match list_runtimes(tools.host_state).await {
            Ok(records) => match serde_json::to_value(records) {
                Ok(value) => success(&request.channel, &request.id, value),
                Err(err) => failure(
                    &request.channel,
                    &request.id,
                    "internal_error",
                    format!("failed to encode runtimes: {err}"),
                ),
            },
            Err(err) => failure(&request.channel, &request.id, "internal_error", err),
        }
    }
}
