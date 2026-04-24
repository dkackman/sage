use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use specta::Type;
use crate::bridge::methods::{BridgeContext, BridgeMethod, BridgeTools};
use crate::bridge::{failure, success, RustBridgeApprovalRequest, RustBridgeRequest, RustBridgeResponse};
use crate::bridge::methods::shared::BridgeMethodCapability;

#[derive(Debug, Clone, Copy)]
pub struct BridgePing;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BridgePingResult {
    pub ok: bool,
    pub app_id: String,
    pub app_name: String,
}

#[async_trait]
impl BridgeMethod for BridgePing {
    fn capability(&self) -> BridgeMethodCapability {
        BridgeMethodCapability::ungated()
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
        let result = BridgePingResult {
            ok: true,
            app_id: ctx.app.id().to_string(),
            app_name: ctx.app.name().to_string(),
        };

        match serde_json::to_value(result) {
            Ok(value) => success(&request.channel, &request.id, value),
            Err(err) => failure(
                &request.channel,
                &request.id,
                "internal_error",
                format!("failed to encode bridge.ping result: {err}"),
            ),
        }
    }
}
