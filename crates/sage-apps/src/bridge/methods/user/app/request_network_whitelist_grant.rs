use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::bridge::methods::{BridgeContext, BridgeMethod, BridgeTools};
use crate::bridge::{
    RustBridgeApprovalRequest,
    RustBridgeRequest, RustBridgeResponse,
};
use crate::bridge::capabilities::UserBridgeCapability;
use crate::bridge::event_emit::emit_bridge_event_to_app_id;
use crate::bridge::methods::shared::BridgeMethodCapability;
use crate::bridge::methods::user::app::{encode_request_success, resolve_app_base_path};
use crate::bridge::methods::user::app::events::EventForApp;
use crate::bridge::types::RustBridgeApprovalBody;
use crate::lifecycle::{
    grant_requested_network_whitelist_entry_internal, parse_network_permission_target,
    GrantNetworkWhitelistOutcome,
};
use crate::types::SageNetworkPermissionTarget;

#[derive(Debug, Clone, Copy)]
pub struct AppRequestNetworkWhitelistGrant;

#[derive(Debug, Clone, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RequestNetworkWhitelistGrantParams {
    pub entry: SageNetworkPermissionTarget,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RequestNetworkWhitelistGrantResult {
    pub granted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub already_granted: Option<bool>,
    pub entry: SageNetworkPermissionTarget,
    pub full_granted_network_whitelist: Vec<SageNetworkPermissionTarget>,
}

fn parse_network_whitelist_grant_params(
    request: &RustBridgeRequest,
) -> Result<RequestNetworkWhitelistGrantParams, RustBridgeResponse> {
    let Some(params_json) = request.params_json.clone() else {
        return Err(RustBridgeResponse::error(
            &request.channel,
            &request.id,
            "invalid_request",
            "sage.requestNetworkWhitelistGrant requires params",
        ));
    };

    let mut params: RequestNetworkWhitelistGrantParams =
        serde_json::from_str(&params_json).map_err(|err| {
            RustBridgeResponse::error(
                &request.channel,
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
        .map_err(|err| RustBridgeResponse::error(&request.channel, &request.id, "invalid_request", err))?;

    params.entry = normalized;
    Ok(params)
}

#[async_trait]
impl BridgeMethod for AppRequestNetworkWhitelistGrant {
    fn capability(&self) -> BridgeMethodCapability {
        BridgeMethodCapability::user(UserBridgeCapability::AppRequestNetworkWhitelistGrant)
    }

    fn approval_request(
        &self,
        ctx: BridgeContext<'_>,
        request: &RustBridgeRequest,
    ) -> Option<RustBridgeApprovalRequest> {
        let Ok(params) = parse_network_whitelist_grant_params(request) else {
            return None;
        };

        if ctx
            .app
            .granted_permissions()
            .network
            .whitelist
            .iter()
            .any(|entry| entry == &params.entry)
        {
            return None;
        }

        Some(RustBridgeApprovalRequest {
            app: ctx.app.clone(),
            source_label: ctx.source_label.to_string(),
            request_id: request.id.clone(),
            body: RustBridgeApprovalBody::NetworkWhitelistGrant {
                entry: params.entry,
            },
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

        let base_path = match resolve_app_base_path(&tools, request) {
            Ok(path) => path,
            Err(err) => return err,
        };

        match grant_requested_network_whitelist_entry_internal(
            &base_path,
            &ctx.app.id(),
            &params.entry,
        ) {
            Ok(GrantNetworkWhitelistOutcome::AlreadyGranted {
                   entry,
                   full_granted_network_whitelist,
               }) => encode_request_success(
                request,
                RequestNetworkWhitelistGrantResult {
                    granted: true,
                    already_granted: Some(true),
                    entry,
                    full_granted_network_whitelist,
                },
                "sage.requestNetworkWhitelistGrant result",
            ),
            Ok(GrantNetworkWhitelistOutcome::Granted { entry, change }) => {
                let full_granted_network_whitelist = change.full.clone();

                let _ = emit_bridge_event_to_app_id(
                    tools.app_handle,
                    ctx.app.id(),
                    EventForApp::from_network_whitelist_change(&request.channel, change)
                ).await;

                encode_request_success(
                    request,
                    RequestNetworkWhitelistGrantResult {
                        granted: true,
                        already_granted: None,
                        entry,
                        full_granted_network_whitelist,
                    },
                    "sage.requestNetworkWhitelistGrant result",
                )
            }
            Err(err) => RustBridgeResponse::error(
                &request.channel,
                &request.id,
                "internal_error",
                format!("failed to grant requested network whitelist entry: {err}"),
            ),
        }
    }
}
