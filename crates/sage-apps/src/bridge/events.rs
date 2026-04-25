use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};
use crate::AppsHostState;
use crate::bridge::{response_channel_for_runtime_kind, RustBridgeApprovalEvent, RustBridgeApprovalRequest, RustBridgeResponse};
use crate::bridge::methods::user::app::{GrantedCapabilitiesChangeEvent, GrantedNetworkWhitelistChangeEvent};
use crate::lifecycle::{GrantedCapabilitiesChange, GrantedNetworkWhitelistChange};
use crate::runtime::state::types::SageAppRuntimeKind;

pub(crate) fn emit_approval_requested(
    app: &AppHandle,
    approval_id: String,
    approval: RustBridgeApprovalRequest,
) -> Result<(), String> {
    let window = app
        .get_window("main")
        .ok_or_else(|| "missing main window".to_string())?;

    window
        .emit(
            "apps:bridge-approval-requested",
            RustBridgeApprovalEvent {
                approval_id,
                approval,
            },
        )
        .map_err(|err| format!("failed to emit approval request event: {err}"))
}

pub(crate) fn emit_bridge_response_to_source(
    app: &AppHandle,
    source_label: &str,
    runtime_kind: SageAppRuntimeKind,
    response: &RustBridgeResponse,
) -> Result<(), String> {
    let host_window = app
        .get_window("main")
        .ok_or_else(|| "missing main window".to_string())?;

    let webview = host_window
        .get_webview(source_label)
        .ok_or_else(|| format!("missing webview for label: {source_label}"))?;

    webview
        .emit(&response_event_for_runtime_kind(runtime_kind), response)
        .map_err(|err| format!("failed to emit bridge response: {err}"))
}

pub(crate) fn emit_bridge_event_to_source(
    app: &AppHandle,
    source_label: &str,
    runtime_kind: SageAppRuntimeKind,
    payload: Value,
) -> Result<(), String> {
    let host_window = app
        .get_window("main")
        .ok_or_else(|| "missing main window".to_string())?;

    let webview = host_window
        .get_webview(source_label)
        .ok_or_else(|| format!("missing webview for label: {source_label}"))?;

    webview
        .emit(&event_event_for_runtime_kind(runtime_kind), payload)
        .map_err(|err| format!("failed to emit bridge event: {err}"))
}

pub(crate) async fn emit_bridge_event_to_app_id(
    app: &AppHandle,
    app_id: &str,
    payload: Value,
) -> Result<(), String> {
    let apps_state = app.state::<AppsHostState>();

    let runtime_id = {
        let runtime_by_app_id = apps_state.runtime.runtime_id_by_app_id.lock().await;
        runtime_by_app_id.get(app_id).cloned()
    };

    let Some(runtime_id) = runtime_id else {
        return Ok(());
    };

    let record = {
        let by_runtime_id = apps_state.runtime.runtime_by_runtime_id.lock().await;
        by_runtime_id.get(&runtime_id).cloned()
    };

    let Some(record) = record else {
        return Ok(());
    };

    emit_bridge_event_to_source(app, &record.webview_label, record.runtime_kind, payload)
}

pub(crate) async fn emit_granted_capabilities_change_for_app(
    app: &AppHandle,
    app_id: &str,
    channel: &str,
    change: GrantedCapabilitiesChange,
) -> Result<(), String> {
    let event = GrantedCapabilitiesChangeEvent::from_change(channel.to_string(), change);
    let payload = serde_json::to_value(event)
        .map_err(|err| format!("failed to encode granted capabilities change event: {err}"))?;

    emit_bridge_event_to_app_id(app, app_id, payload).await
}

pub(crate) async fn emit_granted_network_whitelist_change_for_app(
    app: &AppHandle,
    app_id: &str,
    channel: &str,
    change: GrantedNetworkWhitelistChange,
) -> Result<(), String> {
    let event = GrantedNetworkWhitelistChangeEvent::from_change(channel.to_string(), change);
    let payload = serde_json::to_value(event).map_err(|err| {
        format!("failed to encode granted network whitelist change event: {err}")
    })?;

    emit_bridge_event_to_app_id(app, app_id, payload).await
}

fn response_event_for_runtime_kind(runtime_kind: SageAppRuntimeKind) -> String {
    format!("{}:response", response_channel_for_runtime_kind(runtime_kind))
}

fn event_event_for_runtime_kind(runtime_kind: SageAppRuntimeKind) -> String {
    format!("{}:event", response_channel_for_runtime_kind(runtime_kind))
}

