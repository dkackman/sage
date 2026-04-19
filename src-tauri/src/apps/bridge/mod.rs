use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use specta::Type;
use tauri::{AppHandle, Manager, State};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::apps::permission_registry::require_permission_definition;
use crate::apps::runtime::{apps_assert_bridge_origin, resolve_app};
use crate::apps::state::AppsHostState;
use crate::apps::types::InstalledSageApp;

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
    pub app: InstalledSageApp,
    pub source_label: String,
    pub request_id: String,
    pub params_json: String,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RustBridgeHandleResult {
    Immediate {
        response: RustBridgeResponse,
    },
    ApprovalRequired {
        #[serde(rename = "approvalId")]
        #[specta(rename = "approvalId")]
        approval_id: String,
        approval: RustBridgeApprovalRequest,
    },
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
    app: InstalledSageApp,
    source_label: String,
    request: RustBridgeRequest,
}

#[derive(Default)]
pub struct BridgeState {
    pending_approvals: Mutex<BTreeMap<String, PendingBridgeApproval>>,
}

pub(crate) fn success(id: &str, result: Value) -> RustBridgeResponse {
    RustBridgeResponse::Success(RustBridgeSuccessResponse {
        channel: "sage-bridge".into(),
        bridge_version: "v1".into(),
        id: id.into(),
        ok: true,
        result_json: serde_json::to_string(&result)
            .unwrap_or_else(|_| "null".to_string()),
    })
}

pub(crate) fn failure(id: &str, code: &str, message: impl Into<String>) -> RustBridgeResponse {
    RustBridgeResponse::Error(RustBridgeErrorResponse {
        channel: "sage-bridge".into(),
        bridge_version: "v1".into(),
        id: id.into(),
        ok: false,
        error: RustBridgeErrorPayload {
            code: code.into(),
            message: message.into(),
        },
    })
}

fn validate_request_basics(request: &RustBridgeRequest) -> Result<(), RustBridgeResponse> {
    if request.channel != "sage-bridge" {
        return Err(failure(
            &request.id,
            "invalid_request",
            "Invalid bridge channel",
        ));
    }

    if let Some(version) = &request.bridge_version {
        if version != "v1" {
            return Err(failure(
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
    app: &InstalledSageApp,
    source_label: &str,
    request: &RustBridgeRequest,
) -> RustBridgeResponse {
    let registry = BridgeRegistry::new();

    let Some(method) = registry.get(&request.method) else {
        return failure(
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
                host_state: &app_handle.state::<crate::apps::state::AppsHostState>(),
            },
            request,
        )
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn apps_handle_bridge_request(
    app: AppHandle,
    app_state: State<'_, AppState>,
    apps_state: State<'_, AppsHostState>,
    source_label: String,
    request: RustBridgeRequest,
) -> Result<RustBridgeHandleResult, String> {
    if let Err(response) = validate_request_basics(&request) {
        return Ok(RustBridgeHandleResult::Immediate { response });
    }

    let app_id = match apps_assert_bridge_origin(app.clone(), source_label.clone()) {
        Ok(app_id) => app_id,
        Err(err) => {
            return Ok(RustBridgeHandleResult::Immediate {
                response: failure(
                    &request.id,
                    "permission_denied",
                    format!("Bridge origin denied: {err}"),
                ),
            });
        }
    };

    let base_path = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?;

    let resolved_app = resolve_app(&base_path, &app_id)?;

    let registry = BridgeRegistry::new();

    let Some(method) = registry.get(&request.method) else {
        return Ok(RustBridgeHandleResult::Immediate {
            response: failure(
                &request.id,
                "method_not_found",
                format!("Unknown bridge method: {}", request.method),
            ),
        });
    };

    if let Some(capability_key) = method.permission() {
        let capability_definition = require_permission_definition(capability_key)
            .map_err(|err| {
                format!(
                    "bridge method declared unknown capability {capability_key}: {err}"
                )
            })?;

        if !capability_definition.shared_with_app {
            return Err(format!(
                "bridge method {} declared non-shared capability {}",
                request.method, capability_definition.key
            ));
        }

        if !resolved_app
            .granted_permissions
            .capabilities
            .iter()
            .any(|capability| capability == capability_definition.key)
        {
            return Ok(RustBridgeHandleResult::Immediate {
                response: failure(
                    &request.id,
                    "permission_denied",
                    format!("Permission denied for {}", capability_definition.key),
                ),
            });
        }
    }

    if method.requires_approval(&resolved_app) {
        let approval_id = Uuid::new_v4().to_string();

        {
            let mut pending = apps_state.bridge.pending_approvals.lock().await;
            pending.insert(
                approval_id.clone(),
                PendingBridgeApproval {
                    app: resolved_app.clone(),
                    source_label: source_label.clone(),
                    request: request.clone(),
                },
            );
        }

        return Ok(RustBridgeHandleResult::ApprovalRequired {
            approval_id,
            approval: RustBridgeApprovalRequest {
                kind: "send_xch".into(),
                app: resolved_app,
                source_label,
                request_id: request.id.clone(),
                params_json: request.params_json.clone().unwrap_or_else(|| "null".into()),
            },
        });
    }

    let response =
        execute_bridge_request(&app, &app_state, &resolved_app, &source_label, &request).await;

    Ok(RustBridgeHandleResult::Immediate { response })
}

#[tauri::command]
#[specta::specta]
pub async fn apps_resolve_bridge_approval(
    app: AppHandle,
    app_state: State<'_, AppState>,
    apps_state: State<'_, AppsHostState>,
    args: ResolveBridgeApprovalArgs,
) -> Result<RustBridgeResponse, String> {
    let pending = {
        let mut pending = apps_state.bridge.pending_approvals.lock().await;
        pending.remove(&args.approval_id)
    }
        .ok_or_else(|| format!("unknown approval id: {}", args.approval_id))?;

    if !args.approved {
        return Ok(failure(
            &pending.request.id,
            "user_denied",
            args.reason.unwrap_or_else(|| "User denied the request".into()),
        ));
    }

    Ok(
        execute_bridge_request(
            &app,
            &app_state,
            &pending.app,
            &pending.source_label,
            &pending.request,
        )
            .await,
    )
}
