use tauri::{AppHandle, Manager, State, Webview};
use uuid::Uuid;
use crate::AppsHostState;
use crate::bridge::{registry, response_channel_for_registry_kind, RustBridgeInvokeResult, RustBridgeRequest, RustBridgeResponse};
use crate::bridge::capabilities::{BridgeCapability, SystemBridgeCapability, UserBridgeCapability};
use crate::bridge::comms::sage::events::emit_approval_requested;
use crate::bridge::methods::{BridgeContext, BridgeTools};
use crate::bridge::methods::shared::BridgeMethodCapability;
use crate::bridge::registry::BridgeRegistry;
use crate::bridge::types::PendingBridgeApproval;
use crate::host::AppState;
use crate::permissions::{require_system_capability_definition, require_user_capability_definition, resolve_effective_granted_capabilities};
use crate::runtime::{assert_bridge_origin, resolve_app};
use crate::runtime::state::types::SageAppRuntimeKind;
use crate::types::SageApp;

pub async fn invoke(
    app: AppHandle,
    webview: Webview,
    app_state: State<'_, AppState>,
    request: RustBridgeRequest,
    expected_runtime_kind: Option<SageAppRuntimeKind>,
    registry_kind: registry::BridgeRegistryKind,
) -> Result<RustBridgeInvokeResult, String> {
    let expected_channel = response_channel_for_registry_kind(registry_kind);

    if let Err(response) = validate_request_basics(&request, expected_channel) {
        return Ok(RustBridgeInvokeResult::Immediate { response });
    }

    let source_label = webview.label().to_string();

    let (app_id, runtime_kind) = match assert_bridge_origin(app.clone(), source_label.clone()) {
        Ok(value) => value,
        Err(err) => {
            return Ok(RustBridgeInvokeResult::Immediate {
                response: RustBridgeResponse::error(
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
                response: RustBridgeResponse::error(
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
            response: RustBridgeResponse::error(
                expected_channel,
                &request.id,
                "method_not_found",
                format!("Unknown bridge method: {}", request.method),
            ),
        });
    };

    match method.capability() {
        BridgeMethodCapability::Ungated => {}
        BridgeMethodCapability::Required(capability) => {
            if let Err(response) =
                authorize_method_capability(&app_model, &request, capability)
            {
                return Ok(RustBridgeInvokeResult::Immediate { response });
            }
        }
    }

    if let Some(approval) = method.approval_request(
        BridgeContext {
            app: &app_model,
            source_label: &source_label,
        },
        &request,
    ) {
        let approval_id = Uuid::new_v4().to_string();

        {
            let apps_state = app.state::<AppsHostState>();
            let mut pending = apps_state.bridge.pending_approvals.lock().await;
            pending.insert(
                approval_id.clone(),
                PendingBridgeApproval {
                    app: app_model.clone(),
                    source_label: source_label.to_string(),
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

fn authorize_method_capability(
    app: &SageApp,
    request: &RustBridgeRequest,
    capability: BridgeCapability,
) -> Result<(), RustBridgeResponse> {
    match capability {
        BridgeCapability::User(capability) => {
            let definition =
                require_user_capability_definition(capability).map_err(|err| {
                    RustBridgeResponse::error(
                        &request.channel,
                        &request.id,
                        "internal_error",
                        format!(
                            "bridge method declared unknown user capability {}: {err}",
                            capability.key(),
                        ),
                    )
                })?;

            authorize_user_capability(
                app,
                request,
                capability,
                definition.flags.shared_with_app,
            )
        }

        BridgeCapability::System(capability) => {
            let definition =
                require_system_capability_definition(capability).map_err(|err| {
                    RustBridgeResponse::error(
                        &request.channel,
                        &request.id,
                        "internal_error",
                        format!(
                            "bridge method declared unknown system capability {}: {err}",
                            capability.key(),
                        ),
                    )
                })?;

            authorize_system_capability(
                app,
                request,
                capability,
                definition.flags.shared_with_app,
            )
        }
    }
}

fn authorize_user_capability(
    app: &SageApp,
    request: &RustBridgeRequest,
    capability: UserBridgeCapability,
    shared_with_app: bool,
) -> Result<(), RustBridgeResponse> {
    if !shared_with_app {
        return Err(RustBridgeResponse::error(
            &request.channel,
            &request.id,
            "permission_denied",
            format!("Capability {} is not shared with apps", capability.key()),
        ));
    }

    let effective_capabilities = match app {
        SageApp::User(user_app) => resolve_effective_granted_capabilities(
            &user_app.common.requested_permissions,
            &user_app.common.granted_permissions.capabilities,
        )
            .map_err(|err| {
                RustBridgeResponse::error(
                    &request.channel,
                    &request.id,
                    "internal_error",
                    format!("failed to resolve effective permissions: {err}"),
                )
            })?,
        SageApp::System(_) => app.granted_permissions().capabilities.clone(),
    };

    if !effective_capabilities.contains(&capability) {
        return Err(RustBridgeResponse::error(
            &request.channel,
            &request.id,
            "permission_denied",
            format!("Permission denied for {}", capability.key()),
        ));
    }

    Ok(())
}

fn authorize_system_capability(
    app: &SageApp,
    request: &RustBridgeRequest,
    capability: SystemBridgeCapability,
    shared_with_app: bool,
) -> Result<(), RustBridgeResponse> {
    if !shared_with_app {
        return Err(RustBridgeResponse::error(
            &request.channel,
            &request.id,
            "permission_denied",
            format!("Capability {} is not shared with apps", capability.key()),
        ));
    }

    let granted = app
        .system_granted_permissions()
        .map(|permissions| permissions.capabilities.contains(&capability))
        .unwrap_or(false);

    if !granted {
        return Err(RustBridgeResponse::error(
            &request.channel,
            &request.id,
            "permission_denied",
            format!("Permission denied for {}", capability.key()),
        ));
    }

    Ok(())
}

pub(crate) async fn execute_bridge_request(
    app_handle: &AppHandle,
    app_state: &State<'_, AppState>,
    app: &SageApp,
    source_label: &str,
    request: &RustBridgeRequest,
    registry_kind: registry::BridgeRegistryKind,
) -> RustBridgeResponse {
    let registry = BridgeRegistry::new(registry_kind);

    let Some(method) = registry.get(&request.method) else {
        return RustBridgeResponse::error(
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

fn validate_request_basics(
    request: &RustBridgeRequest,
    expected_channel: &str,
) -> Result<(), RustBridgeResponse> {
    if request.channel != expected_channel {
        return Err(RustBridgeResponse::error(
            expected_channel,
            &request.id,
            "invalid_request",
            "Invalid bridge channel",
        ));
    }

    if let Some(version) = &request.bridge_version {
        if version != "v1" {
            return Err(RustBridgeResponse::error(
                expected_channel,
                &request.id,
                "unsupported_bridge_version",
                format!("Unsupported Sage bridge version: {version}"),
            ));
        }
    }

    Ok(())
}
