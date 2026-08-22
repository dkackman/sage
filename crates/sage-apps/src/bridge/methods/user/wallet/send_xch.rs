use async_trait::async_trait;
use sage_api::SendXch;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
    BridgeApprovalRequestResult, BridgeContext, BridgeHandleResult, BridgeMethod,
    BridgeMethodCapability, BridgeMethodHandleError, BridgeTools, RustBridgeApprovalBody,
    RustBridgeApprovalRequest, RustBridgeRequest, UserBridgeCapability, parse_required_params,
};

#[derive(Debug, Clone, Copy)]
pub struct WalletSendXch;

/// Whether this request needs a user approval step.
fn requires_approval(auto_submit_granted: bool, wallet_protected: bool) -> bool {
    !auto_submit_granted || wallet_protected
}

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

fn parse_amount(value: String) -> sage_api::Amount {
    match value.parse::<u64>() {
        Ok(number) => sage_api::Amount::Number(number),
        Err(_) => sage_api::Amount::String(value),
    }
}

impl From<WalletSendXchParams> for SendXch {
    fn from(v: WalletSendXchParams) -> Self {
        Self {
            address: v.address,
            amount: parse_amount(v.amount),
            fee: parse_amount(v.fee),
            memos: v.memos.unwrap_or_default(),
            clawback: v.clawback,
            auto_submit: true,
            password: None,
        }
    }
}

#[async_trait]
impl BridgeMethod for WalletSendXch {
    fn name(&self) -> &'static str {
        "wallet.sendXch"
    }

    fn capability(&self) -> BridgeMethodCapability {
        BridgeMethodCapability::user(UserBridgeCapability::WalletSendXch)
    }

    fn approval_request(
        &self,
        ctx: BridgeContext<'_>,
        request: &RustBridgeRequest,
    ) -> BridgeApprovalRequestResult {
        let auto_submit_granted = ctx
            .app
            .is_capability_granted(UserBridgeCapability::WalletSendXchAutoSubmit.into());

        if !requires_approval(auto_submit_granted, ctx.password_protected) {
            return Ok(None);
        }

        let params = parse_required_params::<WalletSendXchParams>(self, request)?;

        Ok(Some(RustBridgeApprovalRequest {
            body: RustBridgeApprovalBody::SendXch { summary: params },
        }))
    }

    async fn handle(
        &self,
        _ctx: BridgeContext<'_>,
        tools: BridgeTools<'_>,
        request: &RustBridgeRequest,
    ) -> BridgeHandleResult {
        let params: WalletSendXchParams = parse_required_params(self, request)?;
        let mut req: SendXch = params.into();
        req.password = tools.password.clone();

        let result = tools
            .app_state
            .lock()
            .await
            .send_xch(req)
            .await
            .map_err(|err| {
                BridgeMethodHandleError::internal_error(format!("{} failed: {err}", self.name()))
            })?;

        Ok(Box::new(result))
    }
}

#[cfg(test)]
mod tests {
    /// A password-protected wallet must always produce an approval, even when
    /// the app holds `WalletSendXchAutoSubmit`. Silent auto-submit is
    /// incompatible with password protection: there would be no UI moment in
    /// which to collect the password.
    #[test]
    fn protected_wallet_forces_approval_despite_auto_submit_grant() {
        assert!(super::requires_approval(
            /* auto_submit_granted */ true, /* wallet_protected */ true,
        ));
    }

    #[test]
    fn unprotected_wallet_still_honours_auto_submit_grant() {
        assert!(!super::requires_approval(true, false));
    }

    #[test]
    fn without_the_grant_approval_is_always_required() {
        assert!(super::requires_approval(false, false));
        assert!(super::requires_approval(false, true));
    }
}
