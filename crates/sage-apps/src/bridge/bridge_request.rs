use tauri::{AppHandle, Manager, State, Webview};

use crate::{
    AppState, AppsHostState, BridgeApprovalsChangedEvent, BridgeCapability, BridgeContext,
    BridgeMethod, BridgeMethodCapability, BridgeOrigin, BridgeRegistry, BridgeRegistryKind,
    BridgeTools, PendingBridgeApproval, ResolveBridgeApprovalArgs, RuntimeChangeSet,
    RustBridgeApprovalBody, RustBridgeApprovalRequest, RustBridgeInvokeResult, RustBridgeRequest,
    RustBridgeResponse, SYSTEM_APP_BRIDGE_APPROVAL_ID, SharedSageApp, SystemBridgeCapability,
    UserBridgeCapability, assert_bridge_origin, emit_bridge_response_to_app,
    emit_system_runtime_event_to_listeners, ensure_app_is_enabled_for_scope,
    ensure_approval_expiry_loop, find_runtime_by_app_id_optional, get_system_capability_definition,
    get_user_capability_definition, hide_runtime_inner, list_pending_approvals, resolve_app,
    start_bridge_approval_runtime, sync_bridge_approval_runtime, take_pending_approval,
    unix_timestamp_ms, write_pending_approval,
};

pub(crate) async fn process(
    app_handle: AppHandle,
    webview: Webview,
    app_state: State<'_, AppState>,
    request: RustBridgeRequest,
) -> Result<RustBridgeInvokeResult, String> {
    if let Err(result) = assert_bridge_version(&request) {
        return Ok(result);
    }

    let webview_label = webview.label().to_string();

    let origin = match assert_bridge_origin(&app_handle, &webview_label).await {
        Ok(origin) => origin,
        Err(err) => {
            return Ok(RustBridgeInvokeResult::error(
                &request.id,
                "permission_denied",
                format!("Bridge origin denied: {err}"),
            ));
        }
    };

    process_shared(
        &app_handle,
        &app_state,
        &origin,
        BridgeRegistryKind::User,
        &request,
        false,
        None,
    )
    .await
}

pub(crate) async fn process_system(
    app_handle: AppHandle,
    webview: Webview,
    app_state: State<'_, AppState>,
    request: RustBridgeRequest,
) -> Result<RustBridgeInvokeResult, String> {
    if let Err(result) = assert_bridge_version(&request) {
        return Ok(result);
    }
    let webview_label = webview.label().to_string();

    let origin = match assert_system_bridge_origin(&app_handle, &webview_label).await {
        Ok(origin) => origin,
        Err(err) => {
            return Ok(RustBridgeInvokeResult::error(
                &request.id,
                "permission_denied",
                format!("Bridge origin denied: {err}"),
            ));
        }
    };

    process_shared(
        &app_handle,
        &app_state,
        &origin,
        BridgeRegistryKind::System,
        &request,
        false,
        None,
    )
    .await
}

pub(crate) async fn process_after_approval(
    app_handle: &AppHandle,
    app_state: &State<'_, AppState>,
    apps_state: &State<'_, AppsHostState>,
    args: ResolveBridgeApprovalArgs,
) -> Result<(), String> {
    let pending = take_pending_approval(apps_state, &args.approval_id)
        .await
        .ok_or_else(|| format!("No pending approval with id {}", args.approval_id))?;

    sync_bridge_approval_runtime(app_handle, apps_state).await?;

    let approvals_changed_event =
        BridgeApprovalsChangedEvent::new_from_list(list_pending_approvals(apps_state).await);

    emit_system_runtime_event_to_listeners(app_handle, apps_state, approvals_changed_event).await;

    let app = resolve_app(app_handle, &pending.app_id)
        .await
        .map_err(|err| format!("Failed to resolve app: {err}"))?;

    let origin =
        assert_bridge_origin(app_handle, &app.with_app(SharedSageApp::webview_label)).await?;

    let invoke_result = if !args.approved {
        RustBridgeInvokeResult::error(
            &pending.request.id,
            "user_denied",
            args.reason
                .unwrap_or_else(|| "User denied the request".to_string()),
        )
    } else if unix_timestamp_ms() as u64 > pending.expires_at_ms {
        RustBridgeInvokeResult::error(
            &pending.request.id,
            "approval_timeout",
            "Approval expired before it was resolved".to_string(),
        )
    } else if wallet_binding_violated(app_state, &pending).await {
        RustBridgeInvokeResult::error(
            &pending.request.id,
            "wallet_changed",
            "Active wallet changed since the approval was requested".to_string(),
        )
    } else if !approval_requires_password(&pending) {
        // Nothing here reaches a wallet secret, so there is nothing to unlock.
        process_shared(
            app_handle,
            app_state,
            &origin,
            pending.registry_kind,
            &pending.request,
            true,
            None,
        )
        .await?
    } else {
        // The gate targets the wallet the approval actually acts on, which for
        // `GetSecretKey` is the fingerprint in the approval body rather than
        // the active wallet.
        let target_fingerprint = approval_password_fingerprint(&pending);

        // Only a password-protected wallet can produce a password dialog. On
        // the unprotected path the gate still runs (the frontend may still put
        // a biometric gate in front of it) but nothing covers the main webview,
        // so hiding and re-syncing the approval runtime would be a pure
        // flicker.
        let protected = match target_fingerprint {
            Some(fingerprint) => wallet_password_protected(app_state, fingerprint).await,
            None => active_wallet_password_protected(app_state).await,
        };

        // Hide the approval app before prompting: app runtimes are sibling
        // webviews inside the same window and would cover the main webview's
        // password dialog.
        if protected {
            hide_bridge_approval_runtime(app_handle, apps_state).await;
        }

        let resolved = password_gate_resolve(app_handle, app_state, target_fingerprint).await;

        // The prompt hid the approval runtime, so recompute visibility on every
        // exit path from the password phase -- success, cancel, error, and the
        // post-gate expiry check below alike. Without this a still-queued
        // approval stays invisible with no way for the user to reach it.
        if protected {
            restore_bridge_approval_runtime(app_handle, apps_state).await;
        }

        match resolved {
            // The expiry check above ran before the prompt, and the prompt can
            // outlive the deadline, so the approval window is re-checked here.
            Ok(_) if unix_timestamp_ms() as u64 > pending.expires_at_ms => {
                RustBridgeInvokeResult::error(
                    &pending.request.id,
                    "approval_timeout",
                    "Approval expired during password entry".to_string(),
                )
            }
            Ok(password) => {
                process_shared(
                    app_handle,
                    app_state,
                    &origin,
                    pending.registry_kind,
                    &pending.request,
                    true,
                    password,
                )
                .await?
            }
            Err(err) => RustBridgeInvokeResult::error(&pending.request.id, "unauthorized", err),
        }
    };

    emit_bridge_response_to_app(app_handle, &origin.app, &invoke_result.try_into()?).await?;
    Ok(())
}

async fn process_shared(
    app_handle: &AppHandle,
    app_state: &State<'_, AppState>,
    origin: &BridgeOrigin,
    registry_kind: BridgeRegistryKind,
    request: &RustBridgeRequest,
    approved: bool,
    password: Option<String>,
) -> Result<RustBridgeInvokeResult, String> {
    let registry = BridgeRegistry::new(registry_kind);

    let app = &origin.app;
    if let Err(err) = ensure_app_is_enabled_for_scope(app_state, app).await {
        return Ok(RustBridgeInvokeResult::error(
            &request.id,
            "app_not_enabled_for_scope",
            err,
        ));
    }

    let method = match assert_method(&registry, request) {
        Ok(method) => method,
        Err(response) => return Ok(response.into()),
    };

    match method.capability() {
        BridgeMethodCapability::Ungated => {}

        BridgeMethodCapability::Required(capability) => {
            if let Err(response) = verify_capability(&origin.app, request, capability) {
                return Ok(response.into());
            }
        }
    }

    if approved {
        let response =
            execute_bridge_request(app_handle, app_state, origin, registry, request, password)
                .await;

        return Ok(response.into());
    }

    let password_protected = active_wallet_password_protected(app_state).await;

    match method
        .prepare_approval(
            BridgeContext {
                app,
                password_protected,
            },
            BridgeTools {
                app_handle,
                app_state,
                host_state: &app_handle.state::<AppsHostState>(),
                password: None,
            },
            request,
        )
        .await
    {
        Ok(Some(approval)) => {
            request_approval(
                app_handle,
                app_state,
                app.id(),
                registry_kind,
                approval,
                request,
            )
            .await?;
            Ok(RustBridgeInvokeResult::Pending {})
        }
        Ok(None) => {
            let response =
                execute_bridge_request(app_handle, app_state, origin, registry, request, password)
                    .await;

            Ok(response.into())
        }
        Err(err) => Ok(RustBridgeInvokeResult::error(
            &request.id,
            err.code,
            err.message,
        )),
    }
}

async fn execute_bridge_request(
    app_handle: &AppHandle,
    app_state: &State<'_, AppState>,
    origin: &BridgeOrigin,
    registry: BridgeRegistry,
    request: &RustBridgeRequest,
    password: Option<String>,
) -> RustBridgeResponse {
    let method = match assert_method(&registry, request) {
        Ok(method) => method,
        Err(response) => return response,
    };

    let password_protected = active_wallet_password_protected(app_state).await;

    let result = method
        .handle(
            BridgeContext {
                app: &origin.app,
                password_protected,
            },
            BridgeTools {
                app_handle,
                app_state,
                host_state: &app_handle.state::<AppsHostState>(),
                password,
            },
            request,
        )
        .await;

    match result {
        Ok(value) => match erased_serde::serialize(&*value, serde_json::value::Serializer) {
            Ok(value) => RustBridgeResponse::success(&request.id, &value),
            Err(err) => RustBridgeResponse::error(
                &request.id,
                "internal_error",
                format!("failed to encode {} result: {err}", method.name()),
            ),
        },
        Err(err) => RustBridgeResponse::error(&request.id, err.code, err.message),
    }
}

/// Hides the `bridge-approval` runtime so it cannot cover the `main` webview's
/// password dialog. Best effort: a covered dialog is a UX problem, not a
/// security one, so every failure is logged and the request continues.
async fn hide_bridge_approval_runtime(
    app_handle: &AppHandle,
    apps_state: &State<'_, AppsHostState>,
) {
    let Some(runtime) =
        find_runtime_by_app_id_optional(apps_state, SYSTEM_APP_BRIDGE_APPROVAL_ID).await
    else {
        return;
    };

    let mut changes = RuntimeChangeSet::default();

    if let Err(err) = hide_runtime_inner(app_handle, &runtime, &mut changes) {
        tracing::warn!(
            error = %err,
            "failed to hide the bridge approval runtime before the password prompt"
        );
        return;
    }

    changes.emit(app_handle, apps_state).await;
}

/// Recomputes bridge-approval runtime visibility after the password prompt,
/// re-showing the dialog when further approvals are still queued and letting
/// it stay killed when the queue is empty. Best effort, like the hide: a
/// failure here is logged and never aborts the request.
async fn restore_bridge_approval_runtime(
    app_handle: &AppHandle,
    apps_state: &State<'_, AppsHostState>,
) {
    if let Err(err) = sync_bridge_approval_runtime(app_handle, apps_state).await {
        tracing::warn!(
            error = %err,
            "failed to restore the bridge approval runtime after the password prompt"
        );
    }
}

/// Resolves the master-key password in the trusted `main` webview. The value
/// never crosses into an app runtime: it is handed straight to the wallet
/// method through `BridgeTools`.
async fn password_gate_resolve(
    app_handle: &AppHandle,
    app_state: &State<'_, AppState>,
    fingerprint: Option<u32>,
) -> Result<Option<String>, String> {
    let gate = app_handle.state::<sage_password_gate::PasswordGateState>();

    let resolved = match fingerprint {
        Some(fingerprint) => {
            sage_password_gate::resolve_for_fingerprint(
                app_handle,
                app_state.inner(),
                &gate,
                fingerprint,
            )
            .await
        }
        None => sage_password_gate::resolve(app_handle, app_state.inner(), &gate).await,
    };

    resolved.map_err(|err| err.reason)
}

async fn active_wallet_fingerprint(app_state: &State<'_, AppState>) -> Option<u32> {
    app_state
        .lock()
        .await
        .wallet()
        .map(|wallet| wallet.fingerprint)
        .ok()
}

/// Whether the active wallet is password-protected, per its entry in
/// `sage.wallet_config.wallets`. Deliberately not `Keychain::is_password_protected`:
/// that runs an Argon2 decrypt probe on every call, which is far too
/// expensive for a check made on every bridge request.
async fn active_wallet_password_protected(app_state: &State<'_, AppState>) -> bool {
    app_state
        .lock()
        .await
        .wallet_config()
        .is_some_and(|wallet| wallet.password_protected)
}

/// Whether `fingerprint`'s wallet is password-protected, per its entry in
/// `sage.wallet_config.wallets`. Unlike [`active_wallet_password_protected`]
/// this needs no wallet to be logged in.
async fn wallet_password_protected(app_state: &State<'_, AppState>, fingerprint: u32) -> bool {
    app_state
        .lock()
        .await
        .wallet_config
        .wallets
        .iter()
        .find(|wallet| wallet.fingerprint == fingerprint)
        .is_some_and(|wallet| wallet.password_protected)
}

/// The wallet the password gate must target for this approval.
///
/// `None` means "the active wallet". `GetSecretKey` names its own fingerprint,
/// which need not be the active wallet, so prompting for and verifying the
/// active wallet's password there would hand the keychain the wrong secret.
fn approval_password_fingerprint(pending: &PendingBridgeApproval) -> Option<u32> {
    match pending.approval.body {
        RustBridgeApprovalBody::GetSecretKey { fingerprint } => Some(fingerprint),

        RustBridgeApprovalBody::SendXch { .. }
        | RustBridgeApprovalBody::SignCoinSpends { .. }
        | RustBridgeApprovalBody::SignMessage { .. }
        | RustBridgeApprovalBody::CapabilityGrant { .. }
        | RustBridgeApprovalBody::NetworkWhitelistGrant { .. } => None,
    }
}

/// Whether resuming this approval needs the master-key password.
///
/// Only bodies whose handler reaches a wallet secret are gated. Capability and
/// network-whitelist grants touch no secret, so prompting for them would ask
/// the user for nothing and would fail outright when no wallet is active.
/// Listed exhaustively on purpose: a new approval body must opt into the
/// prompt deliberately rather than inherit one from a catch-all arm.
fn approval_requires_password(pending: &PendingBridgeApproval) -> bool {
    match pending.approval.body {
        RustBridgeApprovalBody::GetSecretKey { .. }
        | RustBridgeApprovalBody::SendXch { .. }
        | RustBridgeApprovalBody::SignCoinSpends { .. }
        | RustBridgeApprovalBody::SignMessage { .. } => true,

        RustBridgeApprovalBody::CapabilityGrant { .. }
        | RustBridgeApprovalBody::NetworkWhitelistGrant { .. } => false,
    }
}

async fn wallet_binding_violated(
    app_state: &State<'_, AppState>,
    pending: &PendingBridgeApproval,
) -> bool {
    let requires_wallet_binding = matches!(
        pending.approval.body,
        RustBridgeApprovalBody::SendXch { .. }
            | RustBridgeApprovalBody::SignCoinSpends { .. }
            | RustBridgeApprovalBody::SignMessage { .. }
    );

    if !requires_wallet_binding {
        return false;
    }

    let Some(approved_fingerprint) = pending.approved_fingerprint else {
        return true;
    };

    active_wallet_fingerprint(app_state).await != Some(approved_fingerprint)
}

async fn request_approval(
    app_handle: &AppHandle,
    app_state: &State<'_, AppState>,
    app_id: String,
    registry_kind: BridgeRegistryKind,
    approval: RustBridgeApprovalRequest,
    request: &RustBridgeRequest,
) -> Result<(), String> {
    let apps_state = app_handle.state::<AppsHostState>();
    let approved_fingerprint = active_wallet_fingerprint(app_state).await;

    write_pending_approval(
        &apps_state,
        app_id.clone(),
        registry_kind,
        &approval,
        request,
        approved_fingerprint,
    )
    .await;

    ensure_approval_expiry_loop(app_handle, &apps_state).await;

    let approvals_changed_event =
        BridgeApprovalsChangedEvent::new_from_list(list_pending_approvals(&apps_state).await);
    emit_system_runtime_event_to_listeners(app_handle, &apps_state, approvals_changed_event).await;

    start_bridge_approval_runtime(app_handle, &apps_state, Vec::from([app_id])).await?;

    Ok(())
}

fn verify_capability(
    app: &SharedSageApp,
    request: &RustBridgeRequest,
    capability: BridgeCapability,
) -> Result<(), RustBridgeResponse> {
    match capability {
        BridgeCapability::User(capability) => {
            let definition = get_user_capability_definition(capability);

            verify_user_capability(
                app,
                request,
                capability,
                definition.flags().shared_with_app(),
            )
        }

        BridgeCapability::System(capability) => {
            let definition = get_system_capability_definition(capability);

            verify_system_capability(
                app,
                request,
                capability,
                definition.flags().shared_with_app(),
            )
        }
    }
}

fn verify_user_capability(
    app: &SharedSageApp,
    request: &RustBridgeRequest,
    capability: UserBridgeCapability,
    shared_with_app: bool,
) -> Result<(), RustBridgeResponse> {
    if !shared_with_app {
        return Err(RustBridgeResponse::error(
            &request.id,
            "permission_denied",
            format!("Capability {} is not shared with apps", capability.key()),
        ));
    }

    let effective_capabilities = app.with(|app| {
        app.common()
            .requested_permissions()
            .capabilities()
            .resolve_effective_grants(app.common().granted_permissions().capabilities().copied())
    });

    if !effective_capabilities.contains(&capability) {
        return Err(RustBridgeResponse::error(
            &request.id,
            "permission_denied",
            format!("Permission denied for {}", capability.key()),
        ));
    }

    Ok(())
}

fn verify_system_capability(
    app: &SharedSageApp,
    request: &RustBridgeRequest,
    capability: SystemBridgeCapability,
    shared_with_app: bool,
) -> Result<(), RustBridgeResponse> {
    if !shared_with_app {
        return Err(RustBridgeResponse::error(
            &request.id,
            "permission_denied",
            format!("Capability {} is not shared with apps", capability.key()),
        ));
    }

    let granted = app.with(|app| {
        app.system_granted_permissions()
            .is_some_and(|permissions| permissions.capabilities().contains(&capability))
    });

    if !granted {
        return Err(RustBridgeResponse::error(
            &request.id,
            "permission_denied",
            format!("Permission denied for {}", capability.key()),
        ));
    }

    Ok(())
}

fn assert_method<'a>(
    registry: &'a BridgeRegistry,
    request: &RustBridgeRequest,
) -> Result<&'a dyn BridgeMethod, RustBridgeResponse> {
    let Some(method) = registry.get(&request.method) else {
        return Err(RustBridgeResponse::error(
            &request.id,
            "method_not_found",
            format!("Unknown bridge method: {}", request.method),
        ));
    };

    Ok(method)
}

async fn assert_system_bridge_origin(
    app_handle: &AppHandle,
    webview_label: &String,
) -> Result<BridgeOrigin, String> {
    let origin = assert_bridge_origin(app_handle, webview_label).await?;

    if !origin.app.is_system_app() {
        return Err("origin app is not a system app".to_string());
    }

    Ok(origin)
}

fn assert_bridge_version(request: &RustBridgeRequest) -> Result<(), RustBridgeInvokeResult> {
    if let Some(version) = &request.bridge_version
        && version != "v1"
    {
        return Err(RustBridgeInvokeResult::error(
            &request.id,
            "unsupported_bridge_version",
            format!("Unsupported Sage bridge version: {version}"),
        ));
    }

    Ok(())
}
