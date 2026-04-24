use async_trait::async_trait;
use sage_api::{GetKeys, GetKeysResponse};

use crate::bridge::capabilities::UserBridgeCapability;
use crate::bridge::methods::shared::BridgeMethodCapability;
use crate::bridge::methods::{BridgeContext, BridgeMethod, BridgeTools};
use crate::bridge::{
    failure, success, RustBridgeApprovalRequest, RustBridgeRequest, RustBridgeResponse,
};

#[derive(Debug, Clone, Copy)]
pub struct WalletGetKeys;

#[async_trait]
impl BridgeMethod for WalletGetKeys {
    fn capability(&self) -> BridgeMethodCapability {
        BridgeMethodCapability::user(UserBridgeCapability::WalletGetKeys)
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
        let result: anyhow::Result<GetKeysResponse> = async {
            let sage = tools.app_state.lock().await;
            sage.get_keys(GetKeys {}).map_err(anyhow::Error::from)
        }
            .await;

        match result {
            Ok(response) => match serde_json::to_value(response) {
                Ok(value) => success(&request.channel, &request.id, value),
                Err(err) => failure(
                    &request.channel,
                    &request.id,
                    "internal_error",
                    format!("failed to encode wallet.getKeys result: {err}"),
                ),
            },
            Err(err) => failure(
                &request.channel,
                &request.id,
                "internal_error",
                format!("wallet.getKeys failed: {err}"),
            ),
        }
    }
}
