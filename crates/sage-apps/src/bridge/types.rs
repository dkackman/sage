use serde::{Deserialize, Serialize};
use specta::Type;
use crate::types::SageApp;

#[derive(Debug, Clone, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RustBridgeRequest {
    pub channel: String,
    pub bridge_version: Option<String>,
    pub id: String,
    pub method: String,
    pub params_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RustBridgeErrorPayload {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RustBridgeSuccessResponse {
    pub channel: String,
    pub bridge_version: String,
    pub id: String,
    pub ok: bool,
    pub result_json: String,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RustBridgeErrorResponse {
    pub channel: String,
    pub bridge_version: String,
    pub id: String,
    pub ok: bool,
    pub error: RustBridgeErrorPayload,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(untagged)]
pub enum RustBridgeResponse {
    Success(RustBridgeSuccessResponse),
    Error(RustBridgeErrorResponse),
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RustBridgeApprovalRequest {
    pub kind: String,
    pub app: SageApp,
    pub source_label: String,
    pub request_id: String,
    pub params_json: String,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RustBridgeApprovalEvent {
    #[serde(rename = "approvalId")]
    #[specta(rename = "approvalId")]
    pub approval_id: String,
    pub approval: RustBridgeApprovalRequest,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RustBridgeInvokeResult {
    Immediate { response: RustBridgeResponse },
    Pending {},
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ResolveBridgeApprovalArgs {
    pub approval_id: String,
    pub approved: bool,
    pub reason: Option<String>,
}
