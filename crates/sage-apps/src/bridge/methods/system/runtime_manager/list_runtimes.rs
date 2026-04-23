use async_trait::async_trait;

use crate::bridge::methods::{BridgeContext, BridgeMethod, BridgeTools};
use crate::bridge::{failure, success, RustBridgeRequest, RustBridgeResponse};

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
        let mut records = {
            let by_runtime_id = tools.host_state.runtime.by_runtime_id.lock().await;
            by_runtime_id.values().cloned().collect::<Vec<_>>()
        };

        records.retain(|record| !record.internal);
        records.sort_by(|a, b| b.started_at.cmp(&a.started_at));

        match serde_json::to_value(records) {
            Ok(value) => success(&request.channel, &request.id, value),
            Err(err) => failure(
                &request.channel,
                &request.id,
                "internal_error",
                format!("failed to encode runtimes: {err}"),
            ),
        }
    }
}
