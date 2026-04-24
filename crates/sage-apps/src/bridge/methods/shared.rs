use async_trait::async_trait;

use crate::bridge::{
    RustBridgeApprovalRequest, RustBridgeRequest, RustBridgeResponse,
};
use crate::bridge::capabilities::{BridgeCapability, SystemBridgeCapability, UserBridgeCapability};
use crate::host::AppState;
use crate::state::AppsHostState;
use crate::types::SageApp;

#[derive(Debug)]
pub struct BridgeContext<'a> {
    pub app: &'a SageApp,
    pub source_label: &'a str,
}

#[derive(Debug)]
pub struct BridgeTools<'a> {
    pub app_handle: &'a tauri::AppHandle,
    pub app_state: &'a tauri::State<'a, AppState>,
    pub host_state: &'a tauri::State<'a, AppsHostState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeMethodCapability {
    Ungated,
    Required(BridgeCapability),
}

#[async_trait]
pub trait BridgeMethod: Send + Sync {
    fn capability(&self) -> BridgeMethodCapability;

    fn approval_request(
        &self,
        ctx: BridgeContext<'_>,
        request: &RustBridgeRequest,
    ) -> Option<RustBridgeApprovalRequest>;

    async fn handle(
        &self,
        ctx: BridgeContext<'_>,
        tools: BridgeTools<'_>,
        request: &RustBridgeRequest,
    ) -> RustBridgeResponse;
}

impl BridgeMethodCapability {
    pub fn ungated() -> Self {
        Self::Ungated
    }

    pub fn user(cap: UserBridgeCapability) -> Self {
        Self::Required(BridgeCapability::User(cap))
    }

    pub fn system(cap: SystemBridgeCapability) -> Self {
        Self::Required(BridgeCapability::System(cap))
    }
}
