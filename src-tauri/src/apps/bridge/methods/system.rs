use std::collections::BTreeSet;
use std::path::PathBuf;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;
use tauri::Manager;

use super::{BridgeContext, BridgeMethod, BridgeTools};
use crate::apps::bridge::{
    emit_bridge_event_to_source, failure, success, RustBridgeApprovalRequest, RustBridgeRequest,
    RustBridgeResponse,
};
use crate::apps::permissions::resolve_shared_capabilities;
use crate::apps::registry::parse_network_permission_target;
use crate::apps::types::SageNetworkPermissionTarget;
use crate::apps::update::{
    grant_requested_capability_internal, grant_requested_network_whitelist_entry_internal,
    GrantCapabilityOutcome, GrantNetworkWhitelistOutcome,
};

#[derive(Debug, Clone, Copy)]
pub struct BridgePing;

#[derive(Debug, Clone, Copy)]
pub struct BridgeSend;

#[derive(Debug, Clone, Copy)]
pub struct AppGetInfo;

#[derive(Debug, Clone, Copy)]
pub struct SageGetCapabilities;

#[derive(Debug, Clone, Copy)]
pub struct SageRequestCapabilityGrant;

#[derive(Debug, Clone, Copy)]
pub struct SageRequestNetworkWhitelistGrant;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequestCapabilityGrantParams {
    capability: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequestNetworkWhitelistGrantParams {
    entry: SageNetworkPermissionTarget,
}

fn parse_capability_grant_params(
    request: &RustBridgeRequest,
) -> Result<RequestCapabilityGrantParams, RustBridgeResponse> {
    let Some(params_json) = request.params_json.clone() else {
        return Err(failure(
            &request.id,
            "invalid_request",
            "sage.requestCapabilityGrant requires params",
        ));
    };

    serde_json::from_str(&params_json).map_err(|err| {
        failure(
            &request.id,
            "invalid_request",
            format!("Failed to decode sage.requestCapabilityGrant params: {err}"),
        )
    })
}

fn parse_network_whitelist_grant_params(
    request: &RustBridgeRequest,
) -> Result<RequestNetworkWhitelistGrantParams, RustBridgeResponse> {
    let Some(params_json) = request.params_json.clone() else {
        return Err(failure(
            &request.id,
            "invalid_request",
            "sage.requestNetworkWhitelistGrant requires params",
        ));
    };

    let mut params: RequestNetworkWhitelistGrantParams =
        serde_json::from_str(&params_json).map_err(|err| {
            failure(
                &request.id,
                "invalid_request",
                format!(
                    "Failed to decode sage.requestNetworkWhitelistGrant params: {err}"
                ),
            )
        })?;

    let normalized = parse_network_permission_target(&format!(
        "{}://{}",
        params.entry.scheme, params.entry.host
    ))
        .map_err(|err| failure(&request.id, "invalid_request", err))?;

    params.entry = normalized;
    Ok(params)
}

fn required_network_set(
    ctx: &BridgeContext<'_>,
) -> Result<BTreeSet<(String, String)>, RustBridgeResponse> {
    let mut out = BTreeSet::new();

    for entry in &ctx.app.requested_permissions.network.whitelist.required {
        let normalized = parse_network_permission_target(&format!("{}://{}", entry.scheme, entry.host))
            .map_err(|err| failure("app.getInfo", "internal_error", err))?;
        out.insert((normalized.scheme, normalized.host));
    }

    Ok(out)
}

fn resolve_app_base_path(
    tools: &BridgeTools<'_>,
    request_id: &str,
) -> Result<PathBuf, RustBridgeResponse> {
    tools
        .app_handle
        .path()
        .app_data_dir()
        .map_err(|err| {
            failure(
                request_id,
                "internal_error",
                format!("failed to resolve app data dir: {err}"),
            )
        })
}

#[async_trait]
impl BridgeMethod for BridgePing {
    async fn handle(
        &self,
        ctx: BridgeContext<'_>,
        _tools: BridgeTools<'_>,
        request: &RustBridgeRequest,
    ) -> RustBridgeResponse {
        success(
            &request.id,
            json!({
                "ok": true,
                "appId": ctx.app.id,
                "appName": ctx.app.name,
            }),
        )
    }
}

#[async_trait]
impl BridgeMethod for BridgeSend {
    async fn handle(
        &self,
        ctx: BridgeContext<'_>,
        tools: BridgeTools<'_>,
        request: &RustBridgeRequest,
    ) -> RustBridgeResponse {
        let payload_json = request
            .params_json
            .clone()
            .unwrap_or_else(|| "null".to_string());

        let payload = serde_json::from_str::<serde_json::Value>(&payload_json)
            .unwrap_or(serde_json::Value::Null);

        crate::apps::sandbox::ingest_bridge_send_payload(
            &ctx.app.id,
            &payload,
            tools.host_state,
        )
            .await;

        success(&request.id, json!({ "ok": true }))
    }
}

#[async_trait]
impl BridgeMethod for AppGetInfo {
    async fn handle(
        &self,
        ctx: BridgeContext<'_>,
        _tools: BridgeTools<'_>,
        request: &RustBridgeRequest,
    ) -> RustBridgeResponse {
        let capabilities =
            resolve_shared_capabilities(&ctx.app.granted_permissions.capabilities)
                .unwrap_or_default();

        let required_network = match required_network_set(&ctx) {
            Ok(value) => value,
            Err(err) => return err,
        };

        let network = ctx
            .app
            .granted_permissions
            .network
            .whitelist
            .iter()
            .map(|entry| {
                json!({
                    "scheme": entry.scheme,
                    "host": entry.host,
                    "required": required_network.contains(&(entry.scheme.clone(), entry.host.clone())),
                })
            })
            .collect::<Vec<_>>();

        success(
            &request.id,
            json!({
                "id": ctx.app.id,
                "name": ctx.app.name,
                "version": ctx.app.version,
                "requestedPermissions": ctx.app.requested_permissions,
                "capabilities": capabilities,
                "network": network,
            }),
        )
    }
}

#[async_trait]
impl BridgeMethod for SageGetCapabilities {
    async fn handle(
        &self,
        ctx: BridgeContext<'_>,
        _tools: BridgeTools<'_>,
        request: &RustBridgeRequest,
    ) -> RustBridgeResponse {
        let capabilities =
            resolve_shared_capabilities(&ctx.app.granted_permissions.capabilities)
                .unwrap_or_default();

        success(
            &request.id,
            serde_json::to_value(&capabilities).unwrap_or_else(|_| json!([])),
        )
    }
}

#[async_trait]
impl BridgeMethod for SageRequestCapabilityGrant {
    fn requires_approval(
        &self,
        app: &crate::apps::types::InstalledSageApp,
        request: &RustBridgeRequest,
    ) -> bool {
        let Ok(params) = parse_capability_grant_params(request) else {
            return false;
        };

        !app
            .granted_permissions
            .capabilities
            .iter()
            .any(|cap| cap == &params.capability)
    }

    fn approval_request(
        &self,
        ctx: BridgeContext<'_>,
        request: &RustBridgeRequest,
    ) -> Option<RustBridgeApprovalRequest> {
        if !self.requires_approval(ctx.app, request) {
            return None;
        }

        let Ok(params) = parse_capability_grant_params(request) else {
            return None;
        };

        Some(RustBridgeApprovalRequest {
            kind: "capability_grant".into(),
            app: ctx.app.clone(),
            source_label: ctx.source_label.to_string(),
            request_id: request.id.clone(),
            params_json: json!({ "capability": params.capability }).to_string(),
        })
    }

    async fn handle(
        &self,
        ctx: BridgeContext<'_>,
        tools: BridgeTools<'_>,
        request: &RustBridgeRequest,
    ) -> RustBridgeResponse {
        let params = match parse_capability_grant_params(request) {
            Ok(params) => params,
            Err(err) => return err,
        };

        let base_path = match resolve_app_base_path(&tools, &request.id) {
            Ok(path) => path,
            Err(err) => return err,
        };

        match grant_requested_capability_internal(&base_path, &ctx.app.id, &params.capability) {
            Ok(GrantCapabilityOutcome::AlreadyGranted {
                   capability,
                   full_granted_capabilities,
               }) => success(
                &request.id,
                json!({
                    "granted": true,
                    "alreadyGranted": true,
                    "capability": capability,
                    "fullGrantedCapabilities": full_granted_capabilities,
                }),
            ),
            Ok(GrantCapabilityOutcome::Granted { capability, change }) => {
                let _ = emit_bridge_event_to_source(
                    tools.app_handle,
                    ctx.source_label,
                    json!({
                        "channel": "sage-bridge",
                        "type": "grantedCapabilitiesChange",
                        "removedGrantedCapabilities": change.removed,
                        "addedGrantedCapabilities": change.added,
                        "fullGrantedCapabilities": change.full,
                    }),
                );

                success(
                    &request.id,
                    json!({
                        "granted": true,
                        "capability": capability,
                        "fullGrantedCapabilities": change.full,
                    }),
                )
            }
            Err(err) => failure(
                &request.id,
                "internal_error",
                format!("failed to grant requested capability: {err}"),
            ),
        }
    }
}

#[async_trait]
impl BridgeMethod for SageRequestNetworkWhitelistGrant {
    fn requires_approval(
        &self,
        app: &crate::apps::types::InstalledSageApp,
        request: &RustBridgeRequest,
    ) -> bool {
        let Ok(params) = parse_network_whitelist_grant_params(request) else {
            return false;
        };

        !app
            .granted_permissions
            .network
            .whitelist
            .iter()
            .any(|entry| entry == &params.entry)
    }

    fn approval_request(
        &self,
        ctx: BridgeContext<'_>,
        request: &RustBridgeRequest,
    ) -> Option<RustBridgeApprovalRequest> {
        if !self.requires_approval(ctx.app, request) {
            return None;
        }

        let Ok(params) = parse_network_whitelist_grant_params(request) else {
            return None;
        };

        Some(RustBridgeApprovalRequest {
            kind: "network_whitelist_grant".into(),
            app: ctx.app.clone(),
            source_label: ctx.source_label.to_string(),
            request_id: request.id.clone(),
            params_json: json!({ "entry": params.entry }).to_string(),
        })
    }

    async fn handle(
        &self,
        ctx: BridgeContext<'_>,
        tools: BridgeTools<'_>,
        request: &RustBridgeRequest,
    ) -> RustBridgeResponse {
        let params = match parse_network_whitelist_grant_params(request) {
            Ok(params) => params,
            Err(err) => return err,
        };

        let base_path = match resolve_app_base_path(&tools, &request.id) {
            Ok(path) => path,
            Err(err) => return err,
        };

        match grant_requested_network_whitelist_entry_internal(
            &base_path,
            &ctx.app.id,
            &params.entry,
        ) {
            Ok(GrantNetworkWhitelistOutcome::AlreadyGranted {
                   entry,
                   full_granted_network_whitelist,
               }) => success(
                &request.id,
                json!({
                    "granted": true,
                    "alreadyGranted": true,
                    "entry": entry,
                    "fullGrantedNetworkWhitelist": full_granted_network_whitelist,
                }),
            ),
            Ok(GrantNetworkWhitelistOutcome::Granted { entry, change }) => {
                let _ = emit_bridge_event_to_source(
                    tools.app_handle,
                    ctx.source_label,
                    json!({
                        "channel": "sage-bridge",
                        "type": "grantedNetworkWhitelistChange",
                        "removedGrantedNetworkWhitelist": change.removed,
                        "addedGrantedNetworkWhitelist": change.added,
                        "fullGrantedNetworkWhitelist": change.full,
                    }),
                );

                success(
                    &request.id,
                    json!({
                        "granted": true,
                        "entry": entry,
                        "fullGrantedNetworkWhitelist": change.full,
                    }),
                )
            }
            Err(err) => failure(
                &request.id,
                "internal_error",
                format!("failed to grant requested network whitelist entry: {err}"),
            ),
        }
    }
}
