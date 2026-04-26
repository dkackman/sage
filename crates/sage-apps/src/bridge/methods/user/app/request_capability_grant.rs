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
use crate::lifecycle::update::types::GrantCapabilityOutcome;
use crate::lifecycle::update::utils::grant_requested_capability_internal;
use crate::permissions::{get_user_capability_definition, user_capability_definition_view};

#[derive(Debug, Clone, Copy)]
pub struct AppRequestCapabilityGrant;

#[derive(Debug, Copy, Clone, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RequestCapabilityGrantParams {
    pub capability: UserBridgeCapability,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RequestCapabilityGrantResult {
    pub granted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub already_granted: Option<bool>,
    pub capability: UserBridgeCapability,
    pub full_granted_capabilities: Vec<UserBridgeCapability>,
}

fn parse_capability_grant_params(
    request: &RustBridgeRequest,
) -> Result<RequestCapabilityGrantParams, RustBridgeResponse> {
    let Some(params_json) = request.params_json.clone() else {
        return Err(RustBridgeResponse::error(
            &request.channel,
            &request.id,
            "invalid_request",
            "sage.requestCapabilityGrant requires params",
        ));
    };

    serde_json::from_str(&params_json).map_err(|err| {
        RustBridgeResponse::error(
            &request.channel,
            &request.id,
            "invalid_request",
            format!("Failed to decode sage.requestCapabilityGrant params: {err}"),
        )
    })
}

#[async_trait]
impl BridgeMethod for AppRequestCapabilityGrant {
    fn capability(&self) -> BridgeMethodCapability {
        BridgeMethodCapability::user(UserBridgeCapability::AppRequestCapabilityGrant)
    }

    fn approval_request(
        &self,
        ctx: BridgeContext<'_>,
        request: &RustBridgeRequest,
    ) -> Option<RustBridgeApprovalRequest> {
        let Ok(params) = parse_capability_grant_params(request) else {
            return None;
        };

        if ctx
            .app
            .granted_permissions()
            .capabilities
            .contains(&params.capability)
        {
            return None;
        }

        let definition = get_user_capability_definition(params.capability)?;

        Some(RustBridgeApprovalRequest {
            app: ctx.app.clone(),
            source_label: ctx.source_label.to_string(),
            request_id: request.id.clone(),
            body: RustBridgeApprovalBody::CapabilityGrant {
                capability: params.capability,
                definition: user_capability_definition_view(definition),
            },
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

        match grant_requested_capability_internal(&base_path, &ctx.app.id(), params.capability) {
            Ok(GrantCapabilityOutcome::AlreadyGranted {
                   capability,
                   full_granted_capabilities,
               }) => encode_request_success(
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

                let _ = emit_bridge_event_to_app_id(
                    tools.app_handle,
                    ctx.app.id(),
                    EventForApp::from_capabilities_change(&request.channel, change)
                ).await;

                encode_request_success(
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
            Err(err) => RustBridgeResponse::error(
                &request.channel,
                &request.id,
                "internal_error",
                format!("failed to grant requested capability: {err}"),
            ),
        }
    }
}
