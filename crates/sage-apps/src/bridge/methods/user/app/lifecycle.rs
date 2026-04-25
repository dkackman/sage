use async_trait::async_trait;

use crate::bridge::methods::{BridgeContext, BridgeMethod, BridgeTools};
use crate::bridge::{failure, success, RustBridgeApprovalRequest, RustBridgeRequest, RustBridgeResponse};
use crate::bridge::capabilities::UserBridgeCapability;
use crate::bridge::methods::shared::BridgeMethodCapability;
use crate::runtime::state::types::{ReadyToStopParams, RuntimeAckResult, SetBeforeStopListenerParams};

#[derive(Debug, Clone, Copy)]
pub struct AppLifecycleSetBeforeStopListener;

#[derive(Debug, Clone, Copy)]
pub struct AppLifecycleReadyToStop;

fn encode_success(
    request: &RustBridgeRequest,
    context: &str,
) -> RustBridgeResponse {
    match serde_json::to_value(RuntimeAckResult { ok: true }) {
        Ok(value) => success(&request.channel, &request.id, value),
        Err(err) => failure(
            &request.channel,
            &request.id,
            "internal_error",
            format!("failed to encode {context}: {err}"),
        ),
    }
}

fn parse_set_before_stop_listener_params(
    request: &RustBridgeRequest,
) -> Result<SetBeforeStopListenerParams, RustBridgeResponse> {
    let Some(params_json) = request.params_json.clone() else {
        return Err(failure(
            &request.channel,
            &request.id,
            "invalid_request",
            "app.lifecycle.setBeforeStopListener requires params",
        ));
    };

    serde_json::from_str(&params_json).map_err(|err| {
        failure(
            &request.channel,
            &request.id,
            "invalid_request",
            format!("Failed to decode app.lifecycle.setBeforeStopListener params: {err}"),
        )
    })
}

fn parse_ready_to_stop_params(
    request: &RustBridgeRequest,
) -> Result<ReadyToStopParams, RustBridgeResponse> {
    let Some(params_json) = request.params_json.clone() else {
        return Err(failure(
            &request.channel,
            &request.id,
            "invalid_request",
            "app.lifecycle.readyToStop requires params",
        ));
    };

    serde_json::from_str(&params_json).map_err(|err| {
        failure(
            &request.channel,
            &request.id,
            "invalid_request",
            format!("Failed to decode app.lifecycle.readyToStop params: {err}"),
        )
    })
}

#[async_trait]
impl BridgeMethod for AppLifecycleSetBeforeStopListener {
    fn capability(&self) -> BridgeMethodCapability {
        BridgeMethodCapability::user(UserBridgeCapability::AppLifecycleSetBeforeStopListener)
    }

    fn approval_request(&self, _ctx: BridgeContext<'_>, _request: &RustBridgeRequest) -> Option<RustBridgeApprovalRequest> {
        None
    }

    async fn handle(
        &self,
        ctx: BridgeContext<'_>,
        tools: BridgeTools<'_>,
        request: &RustBridgeRequest,
    ) -> RustBridgeResponse {
        let params = match parse_set_before_stop_listener_params(request) {
            Ok(value) => value,
            Err(response) => return response,
        };

        let mut listeners = tools
            .host_state
            .runtime
            .before_stop_listeners_by_app_id
            .lock()
            .await;

        if params.active {
            listeners.insert(ctx.app.id().to_string());
        } else {
            listeners.remove(ctx.app.id());
        }

        encode_success(request, "app.lifecycle.setBeforeStopListener result")
    }
}

#[async_trait]
impl BridgeMethod for AppLifecycleReadyToStop {
    fn capability(&self) -> BridgeMethodCapability {
        BridgeMethodCapability::user(UserBridgeCapability::AppLifecycleReadyToStop)
    }

    fn approval_request(&self, _ctx: BridgeContext<'_>, _request: &RustBridgeRequest) -> Option<RustBridgeApprovalRequest> {
        None
    }

    async fn handle(
        &self,
        _ctx: BridgeContext<'_>,
        tools: BridgeTools<'_>,
        request: &RustBridgeRequest,
    ) -> RustBridgeResponse {
        let params = match parse_ready_to_stop_params(request) {
            Ok(value) => value,
            Err(response) => return response,
        };

        let sender = {
            let mut pending = tools.host_state.runtime.pending_stop_ready.lock().await;
            pending.remove(&params.request_id)
        };

        if let Some(sender) = sender {
            let _ = sender.send(());
        }

        encode_success(request, "app.lifecycle.readyToStop result")
    }
}
