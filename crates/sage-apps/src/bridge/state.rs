use tauri::State;
use crate::AppsHostState;
use crate::bridge::RustBridgeRequest;
use crate::bridge::types::PendingBridgeApproval;
use crate::types::SageApp;

pub(crate) async fn write_pending_approval(
    apps_state: &State<'_, AppsHostState>,
    approval_id: &str,
    sage_app: &SageApp,
    webview_label: &str,
    request: &RustBridgeRequest,
) {
    let mut pending = apps_state.bridge.pending_approvals.lock().await;
    pending.insert(
        approval_id.to_string(),
        PendingBridgeApproval {
            app_id: sage_app.id().to_string(),
            webview_label: webview_label.to_string(),
            request: request.clone(),
        },
    );
}

pub(crate) async fn find_pending_approval(
    apps_state: &State<'_, AppsHostState>,
    approval_id: &str,
) -> Option<PendingBridgeApproval> {
    let pending = apps_state.bridge.pending_approvals.lock().await;
    pending.get(approval_id).cloned()
}

pub(crate) async fn remove_pending_approval(
    apps_state: &State<'_, AppsHostState>,
    approval_id: &str,
) {
    let mut pending = apps_state.bridge.pending_approvals.lock().await;
    pending.remove(approval_id);
}