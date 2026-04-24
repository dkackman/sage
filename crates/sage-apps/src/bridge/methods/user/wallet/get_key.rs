use async_trait::async_trait;
use sage_api::{GetKey, GetKeyResponse};

use crate::bridge::capabilities::UserBridgeCapability;
use crate::bridge::methods::shared::BridgeMethodCapability;
use crate::bridge::methods::{BridgeContext, BridgeMethod, BridgeTools};
use crate::bridge::{
    failure, success, RustBridgeApprovalRequest, RustBridgeRequest, RustBridgeResponse,
};

#[derive(Debug, Clone, Copy)]
pub struct WalletGetKey;

fn parse_params(request: &RustBridgeRequest) -> Result<GetKey, RustBridgeResponse> {
    match request.params_json.as_deref() {
        Some(params_json) => serde_json::from_str(params_json).map_err(|err| {
            failure(
                &request.channel,
                &request.id,
                "invalid_request",
                format!("Failed to decode wallet.getKey params: {err}"),
            )
        }),
        None => Ok(GetKey { fingerprint: None }),
    }
}

#[async_trait]
impl BridgeMethod for WalletGetKey {
    fn capability(&self) -> BridgeMethodCapability {
        BridgeMethodCapability::user(UserBridgeCapability::WalletGetKey)
    }

    fn approval_request(
        &self,
        _ctx: BridgeContext<'_>,
        _request: &RustBridgeRequest,
    ) -> Option<RustBridgeApprovalRequest> {
        None
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

        let result: anyhow::Result<GetKeyResponse> = async {
            let sage = tools.app_state.lock().await;
            sage.get_key(params).map_err(anyhow::Error::from)
        }
            .await;

        match result {
            Ok(response) => match serde_json::to_value(response) {
                Ok(value) => success(&request.channel, &request.id, value),
                Err(err) => failure(
                    &request.channel,
                    &request.id,
                    "internal_error",
                    format!("failed to encode wallet.getKey result: {err}"),
                ),
            },
            Err(err) => failure(
                &request.channel,
                &request.id,
                "internal_error",
                format!("wallet.getKey failed: {err}"),
            ),
        }
    }
}
