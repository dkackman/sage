use async_trait::async_trait;

use super::{BridgeContext, BridgeMethod, BridgeTools};
use crate::bridge::{
    failure, success, RustBridgeApprovalRequest, RustBridgeRequest, RustBridgeResponse,
};
use crate::types::SageApp;
use sage_api::SendXch;

#[derive(Debug, Clone, Copy)]
pub struct WalletSendXch;

#[async_trait]
impl BridgeMethod for WalletSendXch {
    fn permission(&self) -> Option<&'static str> {
        Some("wallet.send_xch")
    }

    fn requires_approval(
        &self,
        app: &SageApp,
        _request: &RustBridgeRequest,
    ) -> bool {
        !app
            .granted_permissions()
            .capabilities
            .iter()
            .any(|cap| cap == "wallet.send_xch_auto_submit")
    }

    fn approval_request(
        &self,
        ctx: BridgeContext<'_>,
        request: &RustBridgeRequest,
    ) -> Option<RustBridgeApprovalRequest> {
        if !self.requires_approval(ctx.app, request) {
            return None;
        }

        Some(RustBridgeApprovalRequest {
            kind: "send_xch".into(),
            app: ctx.app.clone(),
            source_label: ctx.source_label.to_string(),
            request_id: request.id.clone(),
            params_json: request.params_json.clone().unwrap_or_else(|| "null".into()),
        })
    }

    async fn handle(
        &self,
        _ctx: BridgeContext<'_>,
        tools: BridgeTools<'_>,
        request: &RustBridgeRequest,
    ) -> RustBridgeResponse {
        let Some(params_json) = request.params_json.clone() else {
            return failure(
                &request.id,
                "invalid_request",
                "wallet.sendXch requires params",
            );
        };

        let mut req: SendXch = match serde_json::from_str(&params_json) {
            Ok(req) => req,
            Err(err) => {
                return failure(
                    &request.id,
                    "invalid_request",
                    format!("Failed to decode wallet.sendXch params: {err}"),
                );
            }
        };

        req.auto_submit = true;

        match tools.app_state.lock().await.send_xch(req).await {
            Ok(result) => match serde_json::to_value(result) {
                Ok(value) => success(&request.id, value),
                Err(err) => failure(
                    &request.id,
                    "internal_error",
                    format!("Failed to encode wallet.sendXch result: {err}"),
                ),
            },
            Err(err) => failure(
                &request.id,
                "internal_error",
                format!("wallet.sendXch failed: {err}"),
            ),
        }
    }
}
