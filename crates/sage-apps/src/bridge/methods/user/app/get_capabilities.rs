use async_trait::async_trait;
use crate::bridge::methods::{BridgeContext, BridgeMethod, BridgeTools};
use crate::bridge::{RustBridgeApprovalRequest, RustBridgeRequest, RustBridgeResponse};
use crate::bridge::capabilities::UserBridgeCapability;
use crate::bridge::methods::shared::BridgeMethodCapability;
use crate::permissions::{resolve_effective_granted_capabilities, resolve_shared_capabilities};
use crate::types::SageApp;

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
        let effective_capabilities = match ctx.app {
            SageApp::User(user_app) => resolve_effective_granted_capabilities(
                &user_app.common.requested_permissions,
                &user_app.common.granted_permissions.capabilities,
            )
                .unwrap_or_default(),
            SageApp::System(_) => ctx.app.granted_permissions().capabilities.clone(),
        };

        let capabilities = resolve_shared_capabilities(&effective_capabilities).unwrap_or_default();

        match serde_json::to_value(&capabilities) {
            Ok(value) => RustBridgeResponse::success(&request.channel, &request.id, value),
            Err(err) => RustBridgeResponse::error(
                &request.channel,
                &request.id,
                "internal_error",
                format!("failed to encode app.getCapabilities result: {err}"),
            ),
        }
    }
}
