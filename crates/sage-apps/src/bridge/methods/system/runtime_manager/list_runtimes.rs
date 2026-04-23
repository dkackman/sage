use async_trait::async_trait;

use crate::bridge::methods::{BridgeContext, BridgeMethod, BridgeTools};
use crate::bridge::{failure, success, RustBridgeRequest, RustBridgeResponse};
use crate::runtime::list_runtimes_internal;

#[derive(Debug, Clone, Copy)]
pub struct SystemListRuntimes;

#[async_trait]
impl BridgeMethod for SystemListRuntimes {
    async fn handle(
        &self,
        _ctx: BridgeContext<'_>,
        tools: BridgeTools<'_>,
        request: &RustBridgeRequest,
    ) -> RustBridgeResponse {
        match list_runtimes_internal(tools.host_state).await {
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
