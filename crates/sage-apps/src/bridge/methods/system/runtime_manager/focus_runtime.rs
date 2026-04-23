use async_trait::async_trait;
use serde_json::json;
use tauri::{Emitter, Manager};

use crate::bridge::methods::{BridgeContext, BridgeMethod, BridgeTools};
use crate::bridge::{failure, success, RustBridgeRequest, RustBridgeResponse};
use crate::bridge::methods::system::runtime_manager::{
    get_runtime_record_by_app_id, now_ms, parse_runtime_target_params, write_runtime_record,
};

#[derive(Debug, Clone, Copy)]
pub struct SystemFocusRuntime;

async fn emit_route_request(
    tools: &BridgeTools<'_>,
    app_id: &str,
) -> Result<(), String> {
    let window = tools
        .app_handle
        .get_window("main")
        .ok_or_else(|| "missing main window".to_string())?;

    window
        .emit(
            "system:route-to-app",
            json!({
                "appId": app_id,
            }),
        )
        .map_err(|err| format!("failed to emit route request: {err}"))
}

#[async_trait]
impl BridgeMethod for SystemFocusRuntime {
    async fn handle(
        &self,
        _ctx: BridgeContext<'_>,
        tools: BridgeTools<'_>,
        request: &RustBridgeRequest,
    ) -> RustBridgeResponse {
        let params = match parse_runtime_target_params(request) {
            Ok(value) => value,
            Err(response) => return response,
        };

        let host_window = match tools.app_handle.get_window("main") {
            Some(window) => window,
            None => {
                return failure(
                    &request.channel,
                    &request.id,
                    "internal_error",
                    "missing main window",
                );
            }
        };

        let mut record = match get_runtime_record_by_app_id(&tools, &params.app_id).await {
            Ok(record) => record,
            Err(err) => {
                return failure(&request.channel, &request.id, "not_found", err);
            }
        };

        let Some(webview) = host_window.get_webview(&record.webview_label) else {
            return failure(
                &request.channel,
                &request.id,
                "not_found",
                format!("missing webview for label: {}", record.webview_label),
            );
        };

        if let Err(err) = emit_route_request(&tools, &params.app_id).await {
            return failure(&request.channel, &request.id, "internal_error", err);
        }

        if let Err(err) = webview.show() {
            return failure(
                &request.channel,
                &request.id,
                "internal_error",
                format!("failed to show webview: {err}"),
            );
        }

        if let Err(err) = webview.set_focus() {
            return failure(
                &request.channel,
                &request.id,
                "internal_error",
                format!("failed to focus webview: {err}"),
            );
        }

        record.visible = true;
        record.state = "running".into();
        record.last_active_at = match now_ms() {
            Ok(value) => value,
            Err(err) => {
                return failure(&request.channel, &request.id, "internal_error", err);
            }
        };

        if let Err(err) = write_runtime_record(&tools, record.clone()).await {
            return failure(&request.channel, &request.id, "internal_error", err);
        }

        match serde_json::to_value(record) {
            Ok(value) => success(&request.channel, &request.id, value),
            Err(err) => failure(
                &request.channel,
                &request.id,
                "internal_error",
                format!("failed to encode runtime record: {err}"),
            ),
        }
    }
}
