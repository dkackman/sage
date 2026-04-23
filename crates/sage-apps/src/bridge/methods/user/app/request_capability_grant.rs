use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use specta::Type;

use crate::bridge::methods::{BridgeContext, BridgeMethod, BridgeTools};
use crate::bridge::{
    emit_granted_capabilities_change_for_app, failure, success, RustBridgeApprovalRequest,
    RustBridgeRequest, RustBridgeResponse,
};
use crate::bridge::methods::user::app::resolve_app_base_path;
use crate::lifecycle::{grant_requested_capability_internal, GrantCapabilityOutcome};

#[derive(Debug, Clone, Copy)]
pub struct SageRequestCapabilityGrant;

#[derive(Debug, Clone, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RequestCapabilityGrantParams {
    pub capability: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RequestCapabilityGrantResult {
    pub granted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub already_granted: Option<bool>,
    pub capability: String,
    pub full_granted_capabilities: Vec<String>,
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

fn parse_capability_grant_params(
    request: &RustBridgeRequest,
) -> Result<RequestCapabilityGrantParams, RustBridgeResponse> {
    let Some(params_json) = request.params_json.clone() else {
        return Err(failure(
            &request.channel,
            &request.id,
            "invalid_request",
            "sage.requestCapabilityGrant requires params",
        ));
    };

    serde_json::from_str(&params_json).map_err(|err| {
        failure(
            &request.channel,
            &request.id,
            "invalid_request",
            format!("Failed to decode sage.requestCapabilityGrant params: {err}"),
        )
    })
}

#[async_trait]
impl BridgeMethod for SageRequestCapabilityGrant {
    fn requires_approval(
        &self,
        app: &crate::types::SageApp,
        request: &RustBridgeRequest,
    ) -> bool {
        let Ok(params) = parse_capability_grant_params(request) else {
            return false;
        };

        !app
            .granted_permissions()
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

        let base_path = match resolve_app_base_path(&tools, request) {
            Ok(path) => path,
            Err(err) => return err,
        };

        match grant_requested_capability_internal(&base_path, &ctx.app.id(), &params.capability) {
            Ok(GrantCapabilityOutcome::AlreadyGranted {
                   capability,
                   full_granted_capabilities,
               }) => encode_success(
                request,
                RequestCapabilityGrantResult {
                    granted: true,
                    already_granted: Some(true),
                    capability,
                    full_granted_capabilities,
                },
                "sage.requestCapabilityGrant result",
            ),
            Ok(GrantCapabilityOutcome::Granted { capability, change }) => {
                let full_granted_capabilities = change.full.clone();

                let _ = emit_granted_capabilities_change_for_app(
                    tools.app_handle,
                    ctx.app.id(),
                    &request.channel,
                    change,
                )
                    .await;

                encode_success(
                    request,
                    RequestCapabilityGrantResult {
                        granted: true,
                        already_granted: None,
                        capability,
                        full_granted_capabilities,
                    },
                    "sage.requestCapabilityGrant result",
                )
            }
            Err(err) => failure(
                &request.channel,
                &request.id,
                "internal_error",
                format!("failed to grant requested capability: {err}"),
            ),
        }
    }
}
