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
use crate::apps::types::{
    SageGrantedPermissions, SageNetworkPermissionTarget, SageRequestedNetworkPermissions,
};
use crate::apps::update::update_app_permissions_internal;

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

fn requested_capability_set(app: &crate::apps::types::InstalledSageApp) -> BTreeSet<String> {
    let mut requested = BTreeSet::new();
    requested.extend(app.requested_permissions.capabilities.required.iter().cloned());
    requested.extend(app.requested_permissions.capabilities.optional.iter().cloned());
    requested
}

fn requested_network_set(
    requested: &SageRequestedNetworkPermissions,
) -> Result<BTreeSet<(String, String)>, String> {
    let mut out = BTreeSet::new();

    for entry in &requested.whitelist.required {
        out.insert(
            parse_network_permission_target(&format!("{}://{}", entry.scheme, entry.host))
                .map(|value| (value.scheme, value.host))?,
        );
    }

    for entry in &requested.whitelist.optional {
        out.insert(
            parse_network_permission_target(&format!("{}://{}", entry.scheme, entry.host))
                .map(|value| (value.scheme, value.host))?,
        );
    }

    Ok(out)
}

fn required_network_set(
    requested: &SageRequestedNetworkPermissions,
) -> Result<BTreeSet<(String, String)>, String> {
    let mut out = BTreeSet::new();

    for entry in &requested.whitelist.required {
        out.insert(
            parse_network_permission_target(&format!("{}://{}", entry.scheme, entry.host))
                .map(|value| (value.scheme, value.host))?,
        );
    }

    Ok(out)
}

fn sort_unique_strings(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let set: BTreeSet<String> = values.into_iter().collect();
    set.into_iter().collect()
}

fn sort_unique_network(
    values: impl IntoIterator<Item = SageNetworkPermissionTarget>,
) -> Vec<SageNetworkPermissionTarget> {
    let set: BTreeSet<SageNetworkPermissionTarget> = values.into_iter().collect();
    set.into_iter().collect()
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

        let required_network = required_network_set(&ctx.app.requested_permissions.network)
            .unwrap_or_default();

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

        !app.granted_permissions.capabilities.iter().any(|cap| cap == &params.capability)
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

        let requested = requested_capability_set(ctx.app);
        if !requested.contains(&params.capability) {
            return failure(
                &request.id,
                "permission_denied",
                format!(
                    "Capability was not requested by app manifest: {}",
                    params.capability
                ),
            );
        }

        if ctx
            .app
            .granted_permissions
            .capabilities
            .iter()
            .any(|cap| cap == &params.capability)
        {
            let full_granted_capabilities = sort_unique_strings(
                ctx.app.granted_permissions.capabilities.clone(),
            );

            return success(
                &request.id,
                json!({
                    "granted": true,
                    "alreadyGranted": true,
                    "capability": params.capability,
                    "fullGrantedCapabilities": full_granted_capabilities,
                }),
            );
        }

        let mut next_capabilities = ctx.app.granted_permissions.capabilities.clone();
        next_capabilities.push(params.capability.clone());
        next_capabilities = sort_unique_strings(next_capabilities);

        let next_granted_permissions = SageGrantedPermissions {
            capabilities: next_capabilities.clone(),
            network: ctx.app.granted_permissions.network.clone(),
        };

        let base_path: PathBuf = match tools.app_handle.path().app_data_dir() {
            Ok(path) => path,
            Err(err) => {
                return failure(
                    &request.id,
                    "internal_error",
                    format!("failed to resolve app data dir: {err}"),
                );
            }
        };

        let updated = match update_app_permissions_internal(
            &base_path,
            &ctx.app.id,
            next_granted_permissions,
            false,
        ) {
            Ok(updated) => updated,
            Err(err) => {
                return failure(
                    &request.id,
                    "internal_error",
                    format!("failed to update granted permissions: {err}"),
                );
            }
        };

        let previous_set: BTreeSet<String> = ctx
            .app
            .granted_permissions
            .capabilities
            .iter()
            .cloned()
            .collect();
        let next_set: BTreeSet<String> = updated
            .granted_permissions
            .capabilities
            .iter()
            .cloned()
            .collect();

        let added_granted_capabilities = next_set
            .difference(&previous_set)
            .cloned()
            .collect::<Vec<_>>();
        let removed_granted_capabilities = previous_set
            .difference(&next_set)
            .cloned()
            .collect::<Vec<_>>();

        let _ = emit_bridge_event_to_source(
            tools.app_handle,
            ctx.source_label,
            json!({
                "channel": "sage-bridge",
                "type": "grantedCapabilitiesChange",
                "removedGrantedCapabilities": removed_granted_capabilities,
                "addedGrantedCapabilities": added_granted_capabilities,
                "fullGrantedCapabilities": updated.granted_permissions.capabilities,
            }),
        );

        success(
            &request.id,
            json!({
                "granted": true,
                "capability": params.capability,
                "fullGrantedCapabilities": updated.granted_permissions.capabilities,
            }),
        )
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

        let requested = match requested_network_set(&ctx.app.requested_permissions.network) {
            Ok(requested) => requested,
            Err(err) => {
                return failure(&request.id, "internal_error", err);
            }
        };

        let entry_key = (params.entry.scheme.clone(), params.entry.host.clone());
        if !requested.contains(&entry_key) {
            return failure(
                &request.id,
                "permission_denied",
                format!(
                    "Network whitelist entry was not requested by app manifest: {}://{}",
                    params.entry.scheme, params.entry.host
                ),
            );
        }

        if ctx
            .app
            .granted_permissions
            .network
            .whitelist
            .iter()
            .any(|entry| entry == &params.entry)
        {
            let full_granted_network_whitelist =
                sort_unique_network(ctx.app.granted_permissions.network.whitelist.clone());

            return success(
                &request.id,
                json!({
                    "granted": true,
                    "alreadyGranted": true,
                    "entry": params.entry,
                    "fullGrantedNetworkWhitelist": full_granted_network_whitelist,
                }),
            );
        }

        let mut next_whitelist = ctx.app.granted_permissions.network.whitelist.clone();
        next_whitelist.push(params.entry.clone());
        next_whitelist = sort_unique_network(next_whitelist);

        let next_granted_permissions = SageGrantedPermissions {
            capabilities: ctx.app.granted_permissions.capabilities.clone(),
            network: crate::apps::types::SageGrantedNetworkPermissions {
                whitelist: next_whitelist.clone(),
            },
        };

        let base_path: PathBuf = match tools.app_handle.path().app_data_dir() {
            Ok(path) => path,
            Err(err) => {
                return failure(
                    &request.id,
                    "internal_error",
                    format!("failed to resolve app data dir: {err}"),
                );
            }
        };

        let updated = match update_app_permissions_internal(
            &base_path,
            &ctx.app.id,
            next_granted_permissions,
            false,
        ) {
            Ok(updated) => updated,
            Err(err) => {
                return failure(
                    &request.id,
                    "internal_error",
                    format!("failed to update granted permissions: {err}"),
                );
            }
        };

        let previous_set: BTreeSet<SageNetworkPermissionTarget> = ctx
            .app
            .granted_permissions
            .network
            .whitelist
            .iter()
            .cloned()
            .collect();
        let next_set: BTreeSet<SageNetworkPermissionTarget> = updated
            .granted_permissions
            .network
            .whitelist
            .iter()
            .cloned()
            .collect();

        let added_granted_network_whitelist = next_set
            .difference(&previous_set)
            .cloned()
            .collect::<Vec<_>>();
        let removed_granted_network_whitelist = previous_set
            .difference(&next_set)
            .cloned()
            .collect::<Vec<_>>();

        let _ = emit_bridge_event_to_source(
            tools.app_handle,
            ctx.source_label,
            json!({
                "channel": "sage-bridge",
                "type": "grantedNetworkWhitelistChange",
                "removedGrantedNetworkWhitelist": removed_granted_network_whitelist,
                "addedGrantedNetworkWhitelist": added_granted_network_whitelist,
                "fullGrantedNetworkWhitelist": updated.granted_permissions.network.whitelist,
            }),
        );

        success(
            &request.id,
            json!({
                "granted": true,
                "entry": params.entry,
                "fullGrantedNetworkWhitelist": updated.granted_permissions.network.whitelist,
            }),
        )
    }
}
