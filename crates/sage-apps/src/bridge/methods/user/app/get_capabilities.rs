use async_trait::async_trait;
use crate::bridge::methods::{BridgeContext, BridgeMethod, BridgeTools};
use crate::bridge::{failure, success, RustBridgeApprovalRequest, RustBridgeRequest, RustBridgeResponse};
use crate::bridge::capabilities::UserBridgeCapability;
use crate::bridge::methods::shared::BridgeMethodCapability;
use crate::permissions::resolve_shared_capabilities;

#[derive(Debug, Clone, Copy)]
pub struct AppGetCapabilities;

#[async_trait]
impl BridgeMethod for AppGetCapabilities {
    fn capability(&self) -> BridgeMethodCapability {
        BridgeMethodCapability::user(UserBridgeCapability::AppGetCapabilities)
    }

    fn approval_request(&self, _ctx: BridgeContext<'_>, _request: &RustBridgeRequest) -> Option<RustBridgeApprovalRequest> {
        None
    }

    async fn handle(
        &self,
        ctx: BridgeContext<'_>,
        _tools: BridgeTools<'_>,
        request: &RustBridgeRequest,
    ) -> RustBridgeResponse {
        let capabilities =
            resolve_shared_capabilities(&ctx.app.granted_permissions().capabilities)
                .unwrap_or_default();

        match serde_json::to_value(&capabilities) {
            Ok(value) => success(&request.channel, &request.id, value),
            Err(err) => failure(
                &request.channel,
                &request.id,
                "internal_error",
                format!("failed to encode sage.getCapabilities result: {err}"),
            ),
        }
    }
}
