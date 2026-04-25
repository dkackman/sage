use tauri::{AppHandle, State, Webview};
use crate::AppsHostState;
use crate::bridge::{registry, response_channel_for_runtime_kind, ResolveBridgeApprovalArgs, RustBridgeInvokeResult, RustBridgeRequest, RustBridgeResponse};
use crate::bridge::app_comms::bridge_request::{execute_bridge_request, process};
use crate::bridge::event_emit::emit_bridge_response_to_source;
use crate::host::AppState;
use crate::permissions::{user_capability_definition_view, user_registry};
use crate::runtime::assert_bridge_origin;
use crate::runtime::state::types::SageAppRuntimeKind;
use crate::types::{SageApp, SageAppCapabilityDefinitionView};

#[tauri::command]
#[specta::specta]
pub async fn apps_invoke_bridge(
    app: AppHandle,
    webview: Webview,
    app_state: State<'_, AppState>,
    request: RustBridgeRequest,
) -> Result<RustBridgeInvokeResult, String> {
    process(
        app,
        webview,
        app_state,
        request,
        None,
        registry::BridgeRegistryKind::User,
    )
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn apps_invoke_system_bridge(
    app: AppHandle,
    webview: Webview,
    app_state: State<'_, AppState>,
    request: RustBridgeRequest,
) -> Result<RustBridgeInvokeResult, String> {
    process(
        app,
        webview,
        app_state,
        request,
        Some(SageAppRuntimeKind::System),
        registry::BridgeRegistryKind::System,
    )
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn apps_resolve_bridge_approval(
    app: AppHandle,
    app_state: State<'_, AppState>,
    apps_state: State<'_, AppsHostState>,
    args: ResolveBridgeApprovalArgs,
) -> Result<(), String> {
    let pending = {
        let mut pending = apps_state.bridge.pending_approvals.lock().await;
        pending.remove(&args.approval_id)
    }
        .ok_or_else(|| format!("unknown approval id: {}", args.approval_id))?;

    let runtime_kind = match assert_bridge_origin(app.clone(), pending.source_label.clone()) {
        Ok((_, runtime_kind)) => runtime_kind,
        Err(_) => match pending.app {
            SageApp::System(_) => SageAppRuntimeKind::System,
            SageApp::User(_) => SageAppRuntimeKind::User,
        },
    };

    let registry_kind = match runtime_kind {
        SageAppRuntimeKind::User => registry::BridgeRegistryKind::User,
        SageAppRuntimeKind::System => registry::BridgeRegistryKind::System,
    };

    let response = if !args.approved {
        RustBridgeResponse::error(
            response_channel_for_runtime_kind(runtime_kind),
            &pending.request.id,
            "user_denied",
            args.reason
                .unwrap_or_else(|| "User denied the request".to_string()),
        )
    } else {
        execute_bridge_request(
            &app,
            &app_state,
            &pending.app,
            &pending.source_label,
            &pending.request,
            registry_kind,
        )
            .await
    };

    emit_bridge_response_to_source(&app, &pending.source_label, runtime_kind, &response)?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn get_user_capability_definitions(
) -> Result<Vec<SageAppCapabilityDefinitionView>, String> {
    Ok(user_registry()
        .values()
        .copied()
        .map(user_capability_definition_view)
        .collect())
}
