use async_trait::async_trait;
use serde_json::json;

use super::{BridgeContext, BridgeMethod, BridgeTools};
use crate::apps::bridge::{success, RustBridgeRequest, RustBridgeResponse};
use crate::apps::permissions::resolve_shared_capabilities;

#[derive(Debug, Clone, Copy)]
pub struct BridgePing;
#[derive(Debug, Clone, Copy)]
pub struct BridgeSend;
#[derive(Debug, Clone, Copy)]
pub struct AppGetInfo;
#[derive(Debug, Clone, Copy)]
pub struct SageGetCapabilities;

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

        success(
            &request.id,
            json!({
                "id": ctx.app.id,
                "name": ctx.app.name,
                "version": ctx.app.version,
                "requestedPermissions": ctx.app.requested_permissions,
                "capabilities": capabilities,
                "network": ctx.app.active_snapshot.manifest.permissions.network.whitelist.required,
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
            serde_json::to_value(&capabilities)
                .unwrap_or_else(|_| json!([])),
        )
    }
}
