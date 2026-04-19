pub mod system;
pub mod wallet;

use async_trait::async_trait;

use crate::app_state::AppState;
use crate::apps::bridge::{RustBridgeRequest, RustBridgeResponse};
use crate::apps::types::InstalledSageApp;

pub struct BridgeContext<'a> {
    pub app: &'a InstalledSageApp,
    pub source_label: &'a str,
}

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

    fn requires_approval(&self, _app: &InstalledSageApp) -> bool {
        false
    }

    async fn handle(
        &self,
        ctx: BridgeContext<'_>,
        tools: BridgeTools<'_>,
        request: &RustBridgeRequest,
    ) -> RustBridgeResponse;
}
