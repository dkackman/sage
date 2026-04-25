use tauri::{AppHandle, Emitter, Manager};
use crate::AppsHostState;
use crate::bridge::{response_channel_for_runtime_kind, RustBridgeResponse};
use crate::bridge::methods::user::app::events::EventForApp;
use crate::runtime::state::read::{find_runtime_by_app_id_optional};
use crate::runtime::state::types::SageAppRuntimeKind;
use crate::runtime::locator::{get_webview_in_sage_window};

pub(crate) fn emit_bridge_response_to_source(
    app: &AppHandle,
    app_webview_label: &str,
    runtime_kind: SageAppRuntimeKind,
    response: &RustBridgeResponse,
) -> Result<(), String> {
    get_webview_in_sage_window(app, app_webview_label)?
        .emit(&response_event_for_runtime_kind(runtime_kind), response)
        .map_err(|err| format!("failed to emit bridge response: {err}"))
}

pub(crate) fn emit_bridge_event_to_source(
    app: &AppHandle,
    app_webview_label: &str,
    runtime_kind: SageAppRuntimeKind,
    event: EventForApp,
) -> Result<(), String> {
    get_webview_in_sage_window(app, app_webview_label)?
        .emit(&event_event_for_runtime_kind(runtime_kind), event)
        .map_err(|err| format!("failed to emit bridge event: {err}"))
}

pub(crate) async fn emit_bridge_event_to_app_id(
    app: &AppHandle,
    app_id: &str,
    event: EventForApp,
) -> Result<(), String> {
    let apps_state = app.state::<AppsHostState>();

    let Some(runtime) = find_runtime_by_app_id_optional(&apps_state, app_id).await else {
        return Ok(());
    };

    emit_bridge_event_to_source(app, &runtime.webview_label, runtime.runtime_kind, event)
}

fn response_event_for_runtime_kind(runtime_kind: SageAppRuntimeKind) -> String {
    format!("{}:response", response_channel_for_runtime_kind(runtime_kind))
}

fn event_event_for_runtime_kind(runtime_kind: SageAppRuntimeKind) -> String {
    format!("{}:event", response_channel_for_runtime_kind(runtime_kind))
}
