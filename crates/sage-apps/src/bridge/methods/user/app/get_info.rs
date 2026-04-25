use std::collections::BTreeSet;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use specta::Type;
use crate::bridge::methods::{BridgeContext, BridgeMethod, BridgeTools};
use crate::bridge::{RustBridgeApprovalRequest, RustBridgeRequest, RustBridgeResponse};
use crate::bridge::capabilities::UserBridgeCapability;
use crate::bridge::methods::shared::BridgeMethodCapability;
use crate::lifecycle::parse_network_permission_target;
use crate::permissions::resolve_shared_capabilities;

#[derive(Debug, Clone, Copy)]
pub struct AppGetInfo;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SageNetworkPermissionInfo {
    pub scheme: String,
    pub host: String,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppGetInfoResult {
    pub id: String,
    pub name: String,
    pub version: String,
    pub requested_permissions: crate::types::SageRequestedPermissions,
    pub capabilities: Vec<UserBridgeCapability>,
    pub network: Vec<SageNetworkPermissionInfo>,
}

#[async_trait]
impl BridgeMethod for AppGetInfo {
    fn capability(&self) -> BridgeMethodCapability {
        BridgeMethodCapability::user(UserBridgeCapability::AppGetInfo)
    }

    fn approval_request(&self, _ctx: BridgeContext<'_>, _request: &RustBridgeRequest) -> Option<RustBridgeApprovalRequest> {
        None
    }

    async fn handle(
        &self,
        ctx: BridgeContext<'_>,
        _tools: BridgeTools<'_>,
        request: &RustBridgeRequest,
    ) -> RustBridgeResponse {
        let capabilities =
            resolve_shared_capabilities(&ctx.app.granted_permissions().capabilities)
                .unwrap_or_default();

        let required_network = match required_network_set(&ctx) {
            Ok(value) => value,
            Err(err) => return err,
        };

        let network = ctx
            .app
            .granted_permissions()
            .network
            .whitelist
            .iter()
            .map(|entry| SageNetworkPermissionInfo {
                scheme: entry.scheme.clone(),
                host: entry.host.clone(),
                required: required_network.contains(&(entry.scheme.clone(), entry.host.clone())),
            })
            .collect::<Vec<_>>();

        let result = AppGetInfoResult {
            id: ctx.app.id().to_string(),
            name: ctx.app.name().to_string(),
            version: ctx.app.version().to_string(),
            requested_permissions: ctx.app.requested_permissions().clone(),
            capabilities,
            network,
        };

        match serde_json::to_value(result) {
            Ok(value) => RustBridgeResponse::success(&request.channel, &request.id, value),
            Err(err) => RustBridgeResponse::error(
                &request.channel,
                &request.id,
                "internal_error",
                format!("failed to encode app.getInfo result: {err}"),
            ),
        }
    }
}

fn required_network_set(
    ctx: &BridgeContext<'_>,
) -> Result<BTreeSet<(String, String)>, RustBridgeResponse> {
    let mut out = BTreeSet::new();

    for entry in &ctx.app.requested_permissions().network.whitelist.required {
        let normalized = parse_network_permission_target(&format!(
            "{}://{}",
            entry.scheme, entry.host
        ))
            .map_err(|err| RustBridgeResponse::error("sage-bridge", "app.getInfo", "internal_error", err))?;

        out.insert((normalized.scheme, normalized.host));
    }

    Ok(out)
}
