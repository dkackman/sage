use tauri::{AppHandle, Emitter};
use crate::bridge::{RustBridgeApprovalEvent, RustBridgeApprovalRequest};
use crate::runtime::locator::{get_sage_webview};

pub(crate) fn emit_approval_requested(
    app: &AppHandle,
    approval_id: String,
    approval: RustBridgeApprovalRequest,
) -> Result<(), String> {
    get_sage_webview(app)?
        .emit(
            "apps:bridge-approval-requested",
            RustBridgeApprovalEvent {
                approval_id,
                approval,
            },
        )
        .map_err(|err| format!("failed to emit approval request event: {err}"))
}
