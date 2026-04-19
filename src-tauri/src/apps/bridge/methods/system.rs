use async_trait::async_trait;
use serde_json::{json};
use tauri::Emitter;

use super::{BridgeContext, BridgeMethod, BridgeTools};
use crate::apps::bridge::{
    success, RustBridgeRequest, RustBridgeResponse, RustSandboxBridgeSendEvent,
};

pub struct BridgePing;
pub struct BridgeSend;
pub struct AppGetInfo;
pub struct SageGetPermissions;

#[async_trait]
impl BridgeMethod for BridgePing {
    fn name(&self) -> &'static str {
        "bridge.ping"
    }

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
    fn name(&self) -> &'static str {
        "bridge.send"
    }

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

        let _ = tools.app_handle.emit(
            "sage-sandbox:report",
            RustSandboxBridgeSendEvent {
                app_id: ctx.app.id.clone(),
                payload_json,
            },
        );

        success(&request.id, json!({ "ok": true }))
    }
}

#[async_trait]
impl BridgeMethod for AppGetInfo {
    fn name(&self) -> &'static str {
        "app.getInfo"
    }

    async fn handle(
        &self,
        ctx: BridgeContext<'_>,
        _tools: BridgeTools<'_>,
        request: &RustBridgeRequest,
    ) -> RustBridgeResponse {
        success(
            &request.id,
            json!({
                "id": ctx.app.id,
                "name": ctx.app.name,
                "version": ctx.app.version,
                "requestedPermissions": ctx.app.requested_permissions,
                "grantedPermissions": ctx.app.granted_permissions.capabilities,
                "network": ctx.app.active_snapshot.manifest.permissions.network.whitelist.required,
            }),
        )
    }
}

#[async_trait]
impl BridgeMethod for SageGetPermissions {
    fn name(&self) -> &'static str {
        "sage.getPermissions"
    }

    async fn handle(
        &self,
        ctx: BridgeContext<'_>,
        _tools: BridgeTools<'_>,
        request: &RustBridgeRequest,
    ) -> RustBridgeResponse {
        success(
            &request.id,
            serde_json::to_value(&ctx.app.granted_permissions.capabilities)
                .unwrap_or_else(|_| json!([])),
        )
    }
}
