use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::bridge::methods::{BridgeContext, BridgeMethod, BridgeTools};
use crate::bridge::{RustBridgeApprovalRequest, RustBridgeRequest, RustBridgeResponse};
use crate::bridge::capabilities::UserBridgeCapability;
use crate::bridge::methods::shared::BridgeMethodCapability;

#[derive(Debug, Clone, Copy)]
pub struct BridgeSend;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeSendRequest {
    pub kind: String,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BridgeSendResult {
    pub ok: bool,
}

fn parse_bridge_send_request(
    request: &RustBridgeRequest,
) -> Result<BridgeSendRequest, RustBridgeResponse> {
    let Some(params_json) = request.params_json.clone() else {
        return Err(RustBridgeResponse::error(
            &request.channel,
            &request.id,
            "invalid_request",
            "bridge.send requires params",
        ));
    };

    serde_json::from_str(&params_json).map_err(|err| {
        RustBridgeResponse::error(
            &request.channel,
            &request.id,
            "invalid_request",
            format!("Failed to decode bridge.send params: {err}"),
        )
    })
}

#[async_trait]
impl BridgeMethod for BridgeSend {
    fn capability(&self) -> BridgeMethodCapability {
        BridgeMethodCapability::user(UserBridgeCapability::BridgeSend)
    }

    fn approval_request(&self, _ctx: BridgeContext<'_>, _request: &RustBridgeRequest) -> Option<RustBridgeApprovalRequest> {
        None
    }

    async fn handle(
        &self,
        ctx: BridgeContext<'_>,
        tools: BridgeTools<'_>,
        request: &RustBridgeRequest,
    ) -> RustBridgeResponse {
        let payload = match parse_bridge_send_request(request) {
            Ok(value) => value,
            Err(response) => return response,
        };

        let payload_value = match serde_json::to_value(&payload) {
            Ok(value) => value,
            Err(err) => {
                return RustBridgeResponse::error(
                    &request.channel,
                    &request.id,
                    "internal_error",
                    format!("failed to encode bridge.send payload: {err}"),
                );
            }
        };

        crate::sandbox::ingest_bridge_send_payload(&ctx.app.id(), &payload_value, tools.host_state)
            .await;

        let result = BridgeSendResult { ok: true };

        match serde_json::to_value(result) {
            Ok(value) => RustBridgeResponse::success(&request.channel, &request.id, value),
            Err(err) => RustBridgeResponse::error(
                &request.channel,
                &request.id,
                "internal_error",
                format!("failed to encode bridge.send result: {err}"),
            ),
        }
    }
}
