use sage_password_gate::{MAX_ATTEMPTS, TOO_MANY_ATTEMPTS_REASON};
use tauri::{AppHandle, Manager, State, Webview};

use crate::{
    AppState, AppsHostState, BridgeApprovalsChangedEvent, BridgeCapability, BridgeContext,
    BridgeMethod, BridgeMethodCapability, BridgeOrigin, BridgeRegistry, BridgeRegistryKind,
    BridgeTools, PendingBridgeApproval, ResolveBridgeApprovalArgs, ResolveBridgeApprovalResult,
    RustBridgeApprovalBody, RustBridgeApprovalRequest, RustBridgeInvokeResult, RustBridgeRequest,
    RustBridgeResponse, SharedSageApp, SystemBridgeCapability, UserBridgeCapability,
    assert_bridge_origin, emit_bridge_response_to_app, emit_system_runtime_event_to_listeners,
    ensure_app_is_enabled_for_scope, ensure_approval_expiry_loop, extend_approval_for_password,
    get_system_capability_definition, get_user_capability_definition, list_pending_approvals,
    peek_pending_approval, record_password_attempt, resolve_app, start_bridge_approval_runtime,
    sync_bridge_approval_runtime, take_pending_approval, unix_timestamp_ms, write_pending_approval,
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
) -> Result<ResolveBridgeApprovalResult, String> {
    // Peeked, not taken: an approval whose password is wrong has to stay queued
    // so the user can try again in the card they are already looking at.
    let pending = peek_pending_approval(apps_state, &args.approval_id)
        .await
        .ok_or_else(|| format!("No pending approval with id {}", args.approval_id))?;

    // Every rejection that does not depend on the password comes first, so the
    // user is never asked to type one into an approval that was already doomed.
    let rejection = if !args.approved {
        Some((
            "user_denied",
            args.reason
                .clone()
                .unwrap_or_else(|| "User denied the request".to_string()),
        ))
    } else if unix_timestamp_ms() as u64 > pending.expires_at_ms {
        Some((
            "approval_timeout",
            "Approval expired before it was resolved".to_string(),
        ))
    } else if wallet_binding_violated(app_state, &pending).await {
        Some((
            "wallet_changed",
            "Active wallet changed since the approval was requested".to_string(),
        ))
    } else {
        None
    };

    if let Some((code, message)) = rejection {
        return consume_and_respond(
            app_handle,
            apps_state,
            &args.approval_id,
            &pending,
            RustBridgeInvokeResult::error(&pending.request.id, code, message),
        )
        .await;
    }

    // Only an approval that reaches a wallet secret *and* targets a protected
    // wallet needs a password; everything else resolves with `None`.
    let password = if approval_needs_password(app_state, &pending.approval.body).await {
        // `GetSecretKey` names its own fingerprint, which need not be the active
        // wallet; verifying the active wallet's password there would check the
        // wrong key.
        let fingerprint = match approval_password_fingerprint(&pending.approval.body) {
            Some(fingerprint) => fingerprint,
            None => active_wallet_fingerprint(app_state)
                .await
                .ok_or_else(|| "No wallet is logged in".to_string())?,
        };

        // The card renders its password field from a hint captured when the
        // approval was queued. If that hint was stale, this is where the card
        // finds out it has to ask
        let Some(candidate) = args.password.as_deref().filter(|it| !it.is_empty()) else {
            if extend_approval_for_password(apps_state, &args.approval_id).await {
                let approvals_changed_event = BridgeApprovalsChangedEvent::new_from_list(
                    list_pending_approvals(apps_state).await,
                );
                emit_system_runtime_event_to_listeners(
                    app_handle,
                    apps_state,
                    approvals_changed_event,
                )
                .await;
            }
            return Ok(ResolveBridgeApprovalResult::PasswordRequired);
        };

        if verify_wallet_password(app_state, fingerprint, candidate).await? {
            Some(candidate.to_string())
        } else {
            let attempts_used = record_password_attempt(apps_state, &args.approval_id)
                .await
                .ok_or_else(|| format!("No pending approval with id {}", args.approval_id))?;

            return match password_attempt_outcome(attempts_used) {
                // Retryable: the approval stays queued and the app keeps waiting.
                PasswordAttempt::Retry { attempts_remaining } => {
                    Ok(ResolveBridgeApprovalResult::WrongPassword { attempts_remaining })
                }
                PasswordAttempt::Exhausted => {
                    consume_and_respond(
                        app_handle,
                        apps_state,
                        &args.approval_id,
                        &pending,
                        RustBridgeInvokeResult::error(
                            &pending.request.id,
                            "unauthorized",
                            TOO_MANY_ATTEMPTS_REASON.to_string(),
                        ),
                    )
                    .await?;

                    Ok(ResolveBridgeApprovalResult::TooManyAttempts)
                }
            };
        }
    } else {
        None
    };

    // Committed: the approval is consumed whatever the wallet method returns.
    // The take is the commit point, and it must actually win: the expiry loop
    // (or a concurrent resolve of the same id) can remove the approval during
    // the password verify above, and executing on the peeked copy after that
    // would broadcast a transaction the app was already told timed out — or
    // execute it twice.
    if take_pending_approval(apps_state, &args.approval_id)
        .await
        .is_none()
    {
        return Err(format!(
            "Approval {} was already resolved or expired",
            args.approval_id
        ));
    }
    finish_approval(app_handle, apps_state).await?;

    let origin = bridge_origin_for(app_handle, &pending).await?;

    // The password, when one was needed, was verified above and is handed
    // straight to the wallet method. It never enters the approval record and
    // never crosses back into an app runtime.
    let invoke_result = process_shared(
        app_handle,
        app_state,
        &origin,
        pending.registry_kind,
        &pending.request,
        true,
        password,
    )
    .await?;

    emit_bridge_response_to_app(app_handle, &origin.app, &invoke_result.try_into()?).await?;

    Ok(ResolveBridgeApprovalResult::Resolved)
}

/// Drops the approval from the queue and hands `invoke_result` back to the app.
async fn consume_and_respond(
    app_handle: &AppHandle,
    apps_state: &State<'_, AppsHostState>,
    approval_id: &str,
    pending: &PendingBridgeApproval,
    invoke_result: RustBridgeInvokeResult,
) -> Result<ResolveBridgeApprovalResult, String> {
    // If the expiry loop (or a concurrent resolve) consumed the approval
    // first, the app already received a response for this request id; sending
    // another would contradict it.
    if take_pending_approval(apps_state, approval_id)
        .await
        .is_none()
    {
        return Err(format!(
            "Approval {approval_id} was already resolved or expired"
        ));
    }
    finish_approval(app_handle, apps_state).await?;

    let origin = bridge_origin_for(app_handle, pending).await?;
    emit_bridge_response_to_app(app_handle, &origin.app, &invoke_result.try_into()?).await?;

    Ok(ResolveBridgeApprovalResult::Resolved)
}

/// Re-syncs the approval runtime and tells listeners the queue changed. Runs
/// once an approval has actually left the queue.
async fn finish_approval(
    app_handle: &AppHandle,
    apps_state: &State<'_, AppsHostState>,
) -> Result<(), String> {
    sync_bridge_approval_runtime(app_handle, apps_state).await?;

    let approvals_changed_event =
        BridgeApprovalsChangedEvent::new_from_list(list_pending_approvals(apps_state).await);

    emit_system_runtime_event_to_listeners(app_handle, apps_state, approvals_changed_event).await;

    Ok(())
}

async fn bridge_origin_for(
    app_handle: &AppHandle,
    pending: &PendingBridgeApproval,
) -> Result<BridgeOrigin, String> {
    let app = resolve_app(app_handle, &pending.app_id)
        .await
        .map_err(|err| format!("Failed to resolve app: {err}"))?;

    assert_bridge_origin(app_handle, &app.with_app(SharedSageApp::webview_label)).await
}

/// Whether `password` unlocks `fingerprint`. Takes the Sage lock only for the
/// probe and never holds it across an await.
async fn verify_wallet_password(
    app_state: &State<'_, AppState>,
    fingerprint: u32,
    password: &str,
) -> Result<bool, String> {
    let sage = app_state.lock().await;

    sage.verify_password(fingerprint, password)
        .map_err(|err| err.to_string())
}

/// What a wrong password means, given how many attempts have now been spent.
/// Shares `MAX_ATTEMPTS` with the native prompt so both paths give the user the
/// same number of tries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PasswordAttempt {
    Retry { attempts_remaining: u8 },
    Exhausted,
}

fn password_attempt_outcome(attempts_used: u8) -> PasswordAttempt {
    match MAX_ATTEMPTS.saturating_sub(attempts_used) {
        0 => PasswordAttempt::Exhausted,
        attempts_remaining => PasswordAttempt::Retry { attempts_remaining },
    }
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
        .is_password_protected_flag(fingerprint)
}

/// Whether this approval must collect a master password: it reaches a wallet
/// secret *and* the wallet it targets is protected.
async fn approval_needs_password(
    app_state: &State<'_, AppState>,
    body: &RustBridgeApprovalBody,
) -> bool {
    if !approval_requires_password(body) {
        return false;
    }

    match approval_password_fingerprint(body) {
        Some(fingerprint) => wallet_password_protected(app_state, fingerprint).await,
        None => active_wallet_password_protected(app_state).await,
    }
}

/// The wallet the password gate must target for this approval.
///
/// `None` means "the active wallet". `GetSecretKey` names its own fingerprint,
/// which need not be the active wallet, so prompting for and verifying the
/// active wallet's password there would hand the keychain the wrong secret.
fn approval_password_fingerprint(body: &RustBridgeApprovalBody) -> Option<u32> {
    match *body {
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
fn approval_requires_password(body: &RustBridgeApprovalBody) -> bool {
    match *body {
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
    let requires_password = approval_needs_password(app_state, &approval.body).await;

    write_pending_approval(
        &apps_state,
        app_id.clone(),
        registry_kind,
        &approval,
        request,
        approved_fingerprint,
        requires_password,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SageNetworkWhitelistEntry;

    /// The user gets `MAX_ATTEMPTS` tries in the approval card, the same budget
    /// the native prompt gives them, and the count shown must be what is left
    /// *after* the attempt they just spent.
    #[test]
    fn a_wrong_password_reports_the_remaining_attempts() {
        assert_eq!(
            password_attempt_outcome(1),
            PasswordAttempt::Retry {
                attempts_remaining: MAX_ATTEMPTS - 1
            }
        );
        assert_eq!(
            password_attempt_outcome(MAX_ATTEMPTS - 1),
            PasswordAttempt::Retry {
                attempts_remaining: 1
            }
        );
    }

    #[test]
    fn spending_the_last_attempt_exhausts_the_approval() {
        assert_eq!(
            password_attempt_outcome(MAX_ATTEMPTS),
            PasswordAttempt::Exhausted
        );
    }

    /// A host that somehow over-counts must still fail closed rather than wrap
    /// around into a fresh budget of retries.
    #[test]
    fn over_counting_attempts_stays_exhausted() {
        assert_eq!(
            password_attempt_outcome(MAX_ATTEMPTS + 10),
            PasswordAttempt::Exhausted
        );
    }

    /// Only the bodies that reach a wallet secret are gated. Capability and
    /// network-whitelist grants must never ask for a password: there is nothing
    /// to unlock, and the prompt would be unanswerable while logged out.
    #[test]
    fn only_secret_bearing_bodies_require_a_password() {
        assert!(approval_requires_password(
            &RustBridgeApprovalBody::GetSecretKey { fingerprint: 1 }
        ));
        assert!(approval_requires_password(
            &RustBridgeApprovalBody::SignMessage {
                message: String::new(),
                public_key: String::new(),
            }
        ));
        assert!(!approval_requires_password(
            &RustBridgeApprovalBody::NetworkWhitelistGrant {
                entry: SageNetworkWhitelistEntry::new_unchecked("https", "example.com"),
                network_id: None,
            }
        ));
    }

    /// Every bridge method that builds one of the password-gated sage-api
    /// request types must inject the resolved password from `BridgeTools`.
    /// The handlers each hand-copy `req.password = tools.password...`; a new
    /// signing method that forgets the line would collect and verify the
    /// user's password in the approval card and then invoke the endpoint with
    /// `password: None` — correct password, "Incorrect password" anyway,
    /// discoverable only at runtime against a protected wallet.
    #[test]
    fn gated_bridge_methods_inject_the_resolved_password() {
        use std::collections::BTreeMap;
        use std::path::Path;

        let manifest: BTreeMap<String, String> =
            serde_json::from_str(include_str!("../../../sage-api/password-gating.json")).unwrap();

        let type_names: Vec<String> = manifest.keys().map(|name| to_pascal_case(name)).collect();

        fn to_pascal_case(snake: &str) -> String {
            snake
                .split('_')
                .map(|word| {
                    let mut characters = word.chars();
                    match characters.next() {
                        Some(first) => {
                            first.to_uppercase().collect::<String>() + characters.as_str()
                        }
                        None => String::new(),
                    }
                })
                .collect()
        }

        /// Whether `token` appears in `source` as a standalone identifier
        /// (so `SendXch` does not match inside `WalletSendXchParams`).
        fn contains_token(source: &str, token: &str) -> bool {
            let is_ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
            source.match_indices(token).any(|(index, _)| {
                let before_ok =
                    index == 0 || !source[..index].chars().next_back().is_some_and(is_ident);
                let after = index + token.len();
                let after_ok = !source[after..].chars().next().is_some_and(is_ident);
                before_ok && after_ok
            })
        }

        fn visit(dir: &Path, files: &mut Vec<(std::path::PathBuf, String)>) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    visit(&path, files);
                } else if path.extension().and_then(|it| it.to_str()) == Some("rs") {
                    files.push((path.clone(), std::fs::read_to_string(&path).unwrap()));
                }
            }
        }

        let methods_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/bridge/methods");
        let mut files = Vec::new();
        visit(&methods_dir, &mut files);
        assert!(files.len() >= 4, "expected bridge method sources");

        let mut missing = Vec::new();
        let mut gated_files = 0;
        for (path, source) in &files {
            if !type_names.iter().any(|name| contains_token(source, name)) {
                continue;
            }
            gated_files += 1;
            if !source.contains("tools.password") {
                missing.push(path.display().to_string());
            }
        }

        // The four wallet signing methods must all be caught, or the scan
        // itself has drifted.
        assert!(
            gated_files >= 4,
            "expected at least 4 bridge method files referencing gated request types, \
             found {gated_files}",
        );
        assert!(
            missing.is_empty(),
            "these bridge methods build a password-gated request type but never inject \
             `tools.password` into it, so the verified password would be dropped: {missing:?}",
        );
    }

    /// `GetSecretKey` acts on the wallet named in its body, which need not be
    /// the active one. Verifying the active wallet's password there would check
    /// the wrong key entirely.
    #[test]
    fn get_secret_key_targets_its_own_fingerprint() {
        assert_eq!(
            approval_password_fingerprint(&RustBridgeApprovalBody::GetSecretKey {
                fingerprint: 1234
            }),
            Some(1234)
        );
        assert_eq!(
            approval_password_fingerprint(&RustBridgeApprovalBody::SignMessage {
                message: String::new(),
                public_key: String::new(),
            }),
            None
        );
    }
}
