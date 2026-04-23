use async_trait::async_trait;
use crate::bridge::methods::{BridgeContext, BridgeMethod, BridgeTools};
use crate::bridge::{failure, success, RustBridgeRequest, RustBridgeResponse};
use crate::permissions::resolve_shared_capabilities;

#[derive(Debug, Clone, Copy)]
pub struct SageGetCapabilities;

#[async_trait]
impl BridgeMethod for SageGetCapabilities {
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
