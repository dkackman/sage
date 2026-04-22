use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Value};
use specta::Type;
use tauri::{AppHandle, Emitter, Manager, State, Webview};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::host::AppState;
use crate::permissions::require_capability_definition;
use crate::state::AppsHostState;
use crate::types::{SageApp};
use crate::runtime::{assert_bridge_origin, resolve_app};
use crate::runtime::records::SageAppRuntimeKind;

pub mod methods;
pub mod registry;

use methods::{BridgeContext, BridgeTools};
use registry::BridgeRegistry;

#[derive(Debug, Clone, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RustBridgeRequest {
    pub channel: String,
    pub bridge_version: Option<String>,
    pub id: String,
    pub method: String,
    pub params_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RustBridgeErrorPayload {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RustBridgeSuccessResponse {
    pub channel: String,
    pub bridge_version: String,
    pub id: String,
    pub ok: bool,
    pub result_json: String,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RustBridgeErrorResponse {
    pub channel: String,
    pub bridge_version: String,
    pub id: String,
    pub ok: bool,
    pub error: RustBridgeErrorPayload,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(untagged)]
pub enum RustBridgeResponse {
    Success(RustBridgeSuccessResponse),
    Error(RustBridgeErrorResponse),
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RustBridgeApprovalRequest {
    pub kind: String,
    pub app: SageApp,
    pub source_label: String,
    pub request_id: String,
    pub params_json: String,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RustBridgeApprovalEvent {
    #[serde(rename = "approvalId")]
    #[specta(rename = "approvalId")]
    pub approval_id: String,
    pub approval: RustBridgeApprovalRequest,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RustBridgeInvokeResult {
    Immediate { response: RustBridgeResponse },
    Pending {},
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ResolveBridgeApprovalArgs {
    pub approval_id: String,
    pub approved: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
struct PendingBridgeApproval {
    app: SageApp,
    source_label: String,
    request: RustBridgeRequest,
}

#[derive(Debug, Default)]
pub struct BridgeState {
    pending_approvals: Mutex<BTreeMap<String, PendingBridgeApproval>>,
}

pub(crate) fn success(
    channel: &str,
    id: &str,
    result: Value,
) -> RustBridgeResponse {
    RustBridgeResponse::Success(RustBridgeSuccessResponse {
        channel: channel.into(),
        bridge_version: "v1".into(),
        id: id.into(),
        ok: true,
        result_json: serde_json::to_string(&result).unwrap_or_else(|_| "null".to_string()),
    })
}

pub(crate) fn failure(
    channel: &str,
    id: &str,
    code: &str,
    message: impl Into<String>,
) -> RustBridgeResponse {
    RustBridgeResponse::Error(RustBridgeErrorResponse {
        channel: channel.into(),
        bridge_version: "v1".into(),
        id: id.into(),
        ok: false,
        error: RustBridgeErrorPayload {
            code: code.into(),
            message: message.into(),
        },
    })
}

fn validate_request_basics(
    request: &RustBridgeRequest,
    expected_channel: &str,
) -> Result<(), RustBridgeResponse> {
    if request.channel != expected_channel {
        return Err(failure(
            expected_channel,
            &request.id,
            "invalid_request",
            "Invalid bridge channel",
        ));
    }

    if let Some(version) = &request.bridge_version {
        if version != "v1" {
            return Err(failure(
                expected_channel,
                &request.id,
                "unsupported_bridge_version",
                format!("Unsupported Sage bridge version: {version}"),
            ));
        }
    }

    Ok(())
}

async fn execute_bridge_request(
    app_handle: &AppHandle,
    app_state: &State<'_, AppState>,
    app: &SageApp,
    source_label: &str,
    request: &RustBridgeRequest,
    registry_kind: registry::BridgeRegistryKind,
) -> RustBridgeResponse {
    let registry = BridgeRegistry::new(registry_kind);

    let Some(method) = registry.get(&request.method) else {
        return failure(
            &request.channel,
            &request.id,
            "method_not_found",
            format!("Unknown bridge method: {}", request.method),
        );
    };

    method
        .handle(
            BridgeContext { app, source_label },
            BridgeTools {
                app_handle,
                app_state,
                host_state: &app_handle.state::<AppsHostState>(),
            },
            request,
        )
        .await
}

fn authorize_method_capability(
    app: &SageApp,
    request: &RustBridgeRequest,
    capability_key: &str,
) -> Result<(), RustBridgeResponse> {
    let capability_definition = require_capability_definition(capability_key)
        .map_err(|err| {
            failure(
                &request.channel,
                &request.id,
                "internal_error",
                format!(
                    "bridge method declared unknown capability {capability_key}: {err}"
                ),
            )
        })?;

    if !capability_definition.shared_with_app {
        return Err(failure(
            &request.channel,
            &request.id,
            "permission_denied",
            format!(
                "Capability {} is not shared with apps",
                capability_definition.key
            ),
        ));
    }

    if !app
        .granted_permissions()
        .capabilities
        .iter()
        .any(|capability| capability == capability_definition.key)
    {
        return Err(failure(
            &request.channel,
            &request.id,
            "permission_denied",
            format!("Permission denied for {}", capability_definition.key),
        ));
    }

    Ok(())
}

fn emit_approval_requested(
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

fn response_channel_for_runtime_kind(runtime_kind: SageAppRuntimeKind) -> &'static str {
    match runtime_kind {
        SageAppRuntimeKind::User => "sage-bridge",
        SageAppRuntimeKind::System => "sage-system-bridge",
    }
}

fn response_event_for_runtime_kind(runtime_kind: SageAppRuntimeKind) -> &'static str {
    match runtime_kind {
        SageAppRuntimeKind::User => "sage-bridge:response",
        SageAppRuntimeKind::System => "sage-system-bridge:response",
    }
}

fn event_event_for_runtime_kind(runtime_kind: SageAppRuntimeKind) -> &'static str {
    match runtime_kind {
        SageAppRuntimeKind::User => "sage-bridge:event",
        SageAppRuntimeKind::System => "sage-system-bridge:event",
    }
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
        .emit(response_event_for_runtime_kind(runtime_kind), response)
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
        .emit(event_event_for_runtime_kind(runtime_kind), payload)
        .map_err(|err| format!("failed to emit bridge event: {err}"))
}

async fn apps_invoke_bridge_internal(
    app: AppHandle,
    webview: Webview,
    app_state: State<'_, AppState>,
    request: RustBridgeRequest,
    expected_runtime_kind: Option<SageAppRuntimeKind>,
    registry_kind: registry::BridgeRegistryKind,
) -> Result<RustBridgeInvokeResult, String> {
    let expected_channel = match registry_kind {
        registry::BridgeRegistryKind::User => "sage-bridge",
        registry::BridgeRegistryKind::System => "sage-system-bridge",
    };

    if let Err(response) = validate_request_basics(&request, expected_channel) {
        return Ok(RustBridgeInvokeResult::Immediate { response });
    }

    let source_label = webview.label().to_string();

    let (app_id, runtime_kind) = match assert_bridge_origin(app.clone(), source_label.clone()) {
        Ok(value) => value,
        Err(err) => {
            return Ok(RustBridgeInvokeResult::Immediate {
                response: failure(
                    expected_channel,
                    &request.id,
                    "permission_denied",
                    format!("Bridge origin denied: {err}"),
                ),
            });
        }
    };

    if let Some(expected_runtime_kind) = expected_runtime_kind {
        if runtime_kind != expected_runtime_kind {
            return Ok(RustBridgeInvokeResult::Immediate {
                response: failure(
                    expected_channel,
                    &request.id,
                    "permission_denied",
                    "This bridge is not available for this runtime kind",
                ),
            });
        }
    }

    let base_path = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?;

    let app_model = resolve_app(&base_path, &app_id)?;

    let registry = BridgeRegistry::new(registry_kind);

    let Some(method) = registry.get(&request.method) else {
        return Ok(RustBridgeInvokeResult::Immediate {
            response: failure(
                expected_channel,
                &request.id,
                "method_not_found",
                format!("Unknown bridge method: {}", request.method),
            ),
        });
    };

    if let Some(permission) = method.permission() {
        if let Err(response) = authorize_method_capability(&app_model, &request, permission) {
            return Ok(RustBridgeInvokeResult::Immediate { response });
        }
    }

    if method.requires_approval(&app_model, &request) {
        let approval = match method.approval_request(
            BridgeContext {
                app: &app_model,
                source_label: &source_label,
            },
            &request,
        ) {
            Some(approval) => approval,
            None => {
                return Ok(RustBridgeInvokeResult::Immediate {
                    response: failure(
                        expected_channel,
                        &request.id,
                        "invalid_request",
                        "Failed to build approval request",
                    ),
                });
            }
        };

        let approval_id = Uuid::new_v4().to_string();

        {
            let bridge_state = app.state::<BridgeState>();
            let mut pending = bridge_state.pending_approvals.lock().await;
            pending.insert(
                approval_id.clone(),
                PendingBridgeApproval {
                    app: app_model.clone(),
                    source_label: source_label.clone(),
                    request: request.clone(),
                },
            );
        }

        emit_approval_requested(&app, approval_id, approval)?;
        return Ok(RustBridgeInvokeResult::Pending {});
    }

    let response = execute_bridge_request(
        &app,
        &app_state,
        &app_model,
        &source_label,
        &request,
        registry_kind,
    )
        .await;

    Ok(RustBridgeInvokeResult::Immediate { response })
}

#[tauri::command]
#[specta::specta]
pub async fn apps_invoke_user_bridge(
    app: AppHandle,
    webview: Webview,
    app_state: State<'_, AppState>,
    request: RustBridgeRequest,
) -> Result<RustBridgeInvokeResult, String> {
    apps_invoke_bridge_internal(
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
    apps_invoke_bridge_internal(
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
        failure(
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
