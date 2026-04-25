use async_trait::async_trait;
use sage_api::{GetSecretKey, GetSecretKeyResponse};

use crate::bridge::capabilities::UserBridgeCapability;
use crate::bridge::methods::shared::BridgeMethodCapability;
use crate::bridge::methods::{BridgeContext, BridgeMethod, BridgeTools};
use crate::bridge::{
    RustBridgeApprovalRequest, RustBridgeRequest, RustBridgeResponse,
};
use crate::bridge::types::RustBridgeApprovalBody;

#[derive(Debug, Clone, Copy)]
pub struct WalletGetSecretKey;

fn parse_params(request: &RustBridgeRequest) -> Result<GetSecretKey, RustBridgeResponse> {
    let Some(params_json) = request.params_json.as_deref() else {
        return Err(RustBridgeResponse::error(
            &request.channel,
            &request.id,
            "invalid_request",
            "wallet.getSecretKey requires params",
        ));
    };

    serde_json::from_str(params_json).map_err(|err| {
        RustBridgeResponse::error(
            &request.channel,
            &request.id,
            "invalid_request",
            format!("Failed to decode wallet.getSecretKey params: {err}"),
        )
    })
}

#[async_trait]
impl BridgeMethod for WalletGetSecretKey {
    fn capability(&self) -> BridgeMethodCapability {
        BridgeMethodCapability::user(UserBridgeCapability::WalletGetSecretKey)
    }

    fn approval_request(
        &self,
        ctx: BridgeContext<'_>,
        request: &RustBridgeRequest,
    ) -> Option<RustBridgeApprovalRequest> {
        let params = match parse_params(request) {
            Ok(params) => params,
            Err(_response) => return None,
        };

        Some(RustBridgeApprovalRequest {
            app: ctx.app.clone(),
            source_label: ctx.source_label.to_string(),
            request_id: request.id.clone(),
            body: RustBridgeApprovalBody::GetSecretKey {
                fingerprint: params.fingerprint,
            },
        })
    }

    async fn handle(
        &self,
        _ctx: BridgeContext<'_>,
        tools: BridgeTools<'_>,
        request: &RustBridgeRequest,
    ) -> RustBridgeResponse {
        let params = match parse_params(request) {
            Ok(params) => params,
            Err(response) => return response,
        };

        let result: anyhow::Result<GetSecretKeyResponse> = async {
            let sage = tools.app_state.lock().await;
            sage.get_secret_key(params).map_err(anyhow::Error::from)
        }
            .await;

        match result {
            Ok(response) => match serde_json::to_value(response) {
                Ok(value) => RustBridgeResponse::success(&request.channel, &request.id, value),
                Err(err) => RustBridgeResponse::error(
                    &request.channel,
                    &request.id,
                    "internal_error",
                    format!("failed to encode wallet.getSecretKey result: {err}"),
                ),
            },
            Err(err) => RustBridgeResponse::error(
                &request.channel,
                &request.id,
                "internal_error",
                format!("wallet.getSecretKey failed: {err}"),
            ),
        }
    }
}
