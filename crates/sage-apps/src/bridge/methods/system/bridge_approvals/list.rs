use async_trait::async_trait;
use serde::Serialize;
use specta::Type;

use crate::{
    BridgeApprovalRequestResult, BridgeContext, BridgeHandleResult, BridgeMethod,
    BridgeMethodCapability, BridgeTools, PendingBridgeApproval, RustBridgeApprovalRequest,
    RustBridgeRequest, SystemBridgeCapability, list_pending_approvals,
};

#[derive(Debug, Clone, Copy)]
pub(crate) struct BridgeApprovalsListPending;

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PendingBridgeApprovalView {
    pub approval_id: String,
    pub app_id: String,
    pub approval: RustBridgeApprovalRequest,
    pub created_at_ms: u64,
    pub expires_at_ms: u64,
    /// Whether the card must collect the master password before approving.
    pub requires_password: bool,
    /// Incorrect attempts already spent, so a card that reloads mid-retry can
    /// still show how many tries are left.
    pub password_attempts: u8,
}

#[async_trait]
impl BridgeMethod for BridgeApprovalsListPending {
    fn name(&self) -> &'static str {
        "bridgeApprovals.listPending"
    }

    fn capability(&self) -> BridgeMethodCapability {
        BridgeMethodCapability::system(SystemBridgeCapability::BridgeApprovalList)
    }

    fn approval_request(
        &self,
        _ctx: BridgeContext<'_>,
        _request: &RustBridgeRequest,
    ) -> BridgeApprovalRequestResult {
        Ok(None)
    }

    async fn handle(
        &self,
        _ctx: BridgeContext<'_>,
        tools: BridgeTools<'_>,
        _request: &RustBridgeRequest,
    ) -> BridgeHandleResult {
        let approvals = list_pending_approvals(tools.host_state)
            .await
            .into_iter()
            .map(PendingBridgeApprovalView::from)
            .collect::<Vec<_>>();
        Ok(Box::new(approvals))
    }
}

impl From<PendingBridgeApproval> for PendingBridgeApprovalView {
    fn from(approval: PendingBridgeApproval) -> Self {
        Self {
            approval_id: approval.approval_id,
            app_id: approval.app_id,
            approval: approval.approval,
            created_at_ms: approval.created_at_ms,
            expires_at_ms: approval.expires_at_ms,
            requires_password: approval.requires_password,
            password_attempts: approval.password_attempts,
        }
    }
}
