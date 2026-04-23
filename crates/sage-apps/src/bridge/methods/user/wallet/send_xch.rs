use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::bridge::methods::{BridgeContext, BridgeMethod, BridgeTools};
use crate::bridge::{
    failure, success, RustBridgeApprovalRequest, RustBridgeRequest, RustBridgeResponse,
};
use crate::types::SageApp;
use sage_api::SendXch;

#[derive(Debug, Clone, Copy)]
pub struct WalletSendXch;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WalletSendXchParams {
    pub address: String,
    pub amount: String,
    pub fee: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memos: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clawback: Option<u64>,
}

fn parse_wallet_send_xch_params(
    request: &RustBridgeRequest,
) -> Result<WalletSendXchParams, RustBridgeResponse> {
    let Some(params_json) = request.params_json.clone() else {
        return Err(failure(
            &request.channel,
            &request.id,
            "invalid_request",
            "wallet.sendXch requires params",
        ));
    };

    serde_json::from_str(&params_json).map_err(|err| {
        failure(
            &request.channel,
            &request.id,
            "invalid_request",
            format!("Failed to decode wallet.sendXch params: {err}"),
        )
    })
}

fn parse_amount(value: String) -> sage_api::Amount {
    match value.parse::<u64>() {
        Ok(number) => sage_api::Amount::Number(number),
        Err(_) => sage_api::Amount::String(value),
    }
}

fn to_send_xch_request(params: WalletSendXchParams) -> SendXch {
    SendXch {
        address: params.address,
        amount: parse_amount(params.amount),
        fee: parse_amount(params.fee),
        memos: params.memos.unwrap_or_default(),
        clawback: params.clawback,
        auto_submit: true,
    }
}

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
        let params = match parse_wallet_send_xch_params(request) {
            Ok(params) => params,
            Err(response) => return response,
        };

        let req = to_send_xch_request(params);

        match tools.app_state.lock().await.send_xch(req).await {
            Ok(result) => match serde_json::to_value(result) {
                Ok(value) => success(&request.channel, &request.id, value),
                Err(err) => failure(
                    &request.channel,
                    &request.id,
                    "internal_error",
                    format!("Failed to encode wallet.sendXch result: {err}"),
                ),
            },
            Err(err) => failure(
                &request.channel,
                &request.id,
                "internal_error",
                format!("wallet.sendXch failed: {err}"),
            ),
        }
    }
}
