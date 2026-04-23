use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use specta::Type;

use crate::bridge::methods::{BridgeContext, BridgeMethod, BridgeTools};
use crate::bridge::{
    emit_granted_network_whitelist_change_for_app, failure, success, RustBridgeApprovalRequest,
    RustBridgeRequest, RustBridgeResponse,
};
use crate::bridge::methods::user::app::resolve_app_base_path;
use crate::lifecycle::{
    grant_requested_network_whitelist_entry_internal, parse_network_permission_target,
    GrantNetworkWhitelistOutcome,
};
use crate::types::SageNetworkPermissionTarget;

#[derive(Debug, Clone, Copy)]
pub struct SageRequestNetworkWhitelistGrant;

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

fn encode_success<T: Serialize>(
    request: &RustBridgeRequest,
    value: T,
    context: &str,
) -> RustBridgeResponse {
    match serde_json::to_value(value) {
        Ok(value) => success(&request.channel, &request.id, value),
        Err(err) => failure(
            &request.channel,
            &request.id,
            "internal_error",
            format!("failed to encode {context}: {err}"),
        ),
    }
}

fn parse_network_whitelist_grant_params(
    request: &RustBridgeRequest,
) -> Result<RequestNetworkWhitelistGrantParams, RustBridgeResponse> {
    let Some(params_json) = request.params_json.clone() else {
        return Err(failure(
            &request.channel,
            &request.id,
            "invalid_request",
            "sage.requestNetworkWhitelistGrant requires params",
        ));
    };

    let mut params: RequestNetworkWhitelistGrantParams =
        serde_json::from_str(&params_json).map_err(|err| {
            failure(
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
        .map_err(|err| failure(&request.channel, &request.id, "invalid_request", err))?;

    params.entry = normalized;
    Ok(params)
}

#[async_trait]
impl BridgeMethod for SageRequestNetworkWhitelistGrant {
    fn requires_approval(
        &self,
        app: &crate::types::SageApp,
        request: &RustBridgeRequest,
    ) -> bool {
        let Ok(params) = parse_network_whitelist_grant_params(request) else {
            return false;
        };

        !app
            .granted_permissions()
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
               }) => encode_success(
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

                let _ = emit_granted_network_whitelist_change_for_app(
                    tools.app_handle,
                    ctx.app.id(),
                    &request.channel,
                    change,
                )
                    .await;

                encode_success(
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
            Err(err) => failure(
                &request.channel,
                &request.id,
                "internal_error",
                format!("failed to grant requested network whitelist entry: {err}"),
            ),
        }
    }
}
