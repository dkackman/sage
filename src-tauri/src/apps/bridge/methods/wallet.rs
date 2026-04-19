use async_trait::async_trait;

use super::{BridgeContext, BridgeMethod, BridgeTools};
use crate::apps::bridge::{failure, success, RustBridgeRequest, RustBridgeResponse};
use crate::apps::types::InstalledSageApp;

// If this import fails, adjust only this line.
use sage_api::SendXch;

pub struct WalletSendXch;

#[async_trait]
impl BridgeMethod for WalletSendXch {
    fn permission(&self) -> Option<&'static str> {
        Some("wallet.send_xch")
    }

    fn requires_approval(&self, app: &InstalledSageApp) -> bool {
        !app
            .granted_permissions
            .capabilities
            .iter()
            .any(|cap| cap == "wallet.send_xch_auto_submit")
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

        match crate::commands::send_xch(tools.app_state.clone(), req).await {
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
