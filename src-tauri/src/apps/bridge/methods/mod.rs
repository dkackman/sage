pub mod system;
pub mod wallet;

use async_trait::async_trait;

use crate::app_state::AppState;
use crate::apps::bridge::{RustBridgeApprovalRequest, RustBridgeRequest, RustBridgeResponse};
use crate::apps::types::InstalledSageApp;

#[derive(Debug)]
pub struct BridgeContext<'a> {
    pub app: &'a InstalledSageApp,
    pub source_label: &'a str,
}

#[derive(Debug)]
pub struct BridgeTools<'a> {
    pub app_handle: &'a tauri::AppHandle,
    pub app_state: &'a tauri::State<'a, AppState>,
    pub host_state: &'a tauri::State<'a, crate::apps::state::AppsHostState>,
}

#[async_trait]
pub trait BridgeMethod: Send + Sync {
    fn permission(&self) -> Option<&'static str> {
        None
    }

    fn requires_approval(
        &self,
        _app: &InstalledSageApp,
        _request: &RustBridgeRequest,
    ) -> bool {
        false
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
            kind: "unknown".into(),
            app: ctx.app.clone(),
            source_label: ctx.source_label.to_string(),
            request_id: request.id.clone(),
            params_json: request
                .params_json
                .clone()
                .unwrap_or_else(|| "null".to_string()),
        })
    }

    async fn handle(
        &self,
        ctx: BridgeContext<'_>,
        tools: BridgeTools<'_>,
        request: &RustBridgeRequest,
    ) -> RustBridgeResponse;
}
