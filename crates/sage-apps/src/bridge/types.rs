use serde::{Deserialize, Serialize};
use serde_json::Value;
use specta::Type;

use crate::{
    BridgeRegistryKind, SageAppCapabilityDefinitionView, SageNetworkWhitelistEntry, SharedSageApp,
    UserBridgeCapability, WalletSendXchParams, WalletSignCoinSpendsApprovalSummary,
};

#[derive(Debug, Clone, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RustBridgeRequest {
    pub bridge_version: Option<String>,
    pub id: String,
    pub method: String,
    pub params_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RustBridgeInvokeResult {
    Success(RustBridgeSuccessResponse),
    Error(RustBridgeErrorResponse),
    Pending,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(untagged)]
pub enum RustBridgeResponse {
    Success(RustBridgeSuccessResponse),
    Error(RustBridgeErrorResponse),
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RustBridgeSuccessResponse {
    pub bridge_version: String,
    pub id: String,
    pub ok: bool,
    pub result_json: String,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RustBridgeErrorResponse {
    pub bridge_version: String,
    pub id: String,
    pub ok: bool,
    pub error: RustBridgeErrorPayload,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RustBridgeErrorPayload {
    pub code: String,
    pub message: String,
}

#[derive(Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ResolveBridgeApprovalArgs {
    pub approval_id: String,
    pub approved: bool,
    pub reason: Option<String>,
    /// Master-key password, collected inline by the bridge-approval card when
    /// the approval targets a password-protected wallet. Never stored, never
    /// forwarded to an app.
    #[serde(default)]
    pub password: Option<String>,
}

/// Hand-written so the master password can never reach a log line, matching
/// `BridgeTools`. The `Some`/`None` distinction is kept; the value never is.
impl std::fmt::Debug for ResolveBridgeApprovalArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolveBridgeApprovalArgs")
            .field("approval_id", &self.approval_id)
            .field("approved", &self.approved)
            .field("reason", &self.reason)
            .field("password", &self.password.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

/// What `bridgeApprovals.resolve` tells the approval card.
///
/// A wrong password is not an error: the approval stays queued so the user can
/// retry in place, exactly as the native prompt allows. Only when the attempts
/// run out is the approval consumed and the app's request rejected.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ResolveBridgeApprovalResult {
    /// The approval was consumed: approved and executed, denied, or failed.
    Resolved,
    /// The approval needs a password and none was supplied. No attempt spent.
    PasswordRequired,
    /// Wrong password; the approval is still queued.
    WrongPassword {
        #[serde(rename = "attemptsRemaining")]
        attempts_remaining: u8,
    },
    /// Wrong password too many times; the approval is gone and the app's
    /// request was rejected.
    TooManyAttempts,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RustBridgeApprovalRequest {
    #[serde(flatten)]
    pub body: RustBridgeApprovalBody,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RustBridgeApprovalBody {
    GetSecretKey {
        fingerprint: u32,
    },
    SendXch {
        summary: WalletSendXchParams,
    },
    SignCoinSpends {
        summary: WalletSignCoinSpendsApprovalSummary,
        #[serde(rename = "partialSign")]
        partial_sign: bool,
    },
    SignMessage {
        message: String,
        #[serde(rename = "publicKey")]
        public_key: String,
    },
    CapabilityGrant {
        capability: UserBridgeCapability,
        definition: SageAppCapabilityDefinitionView,
    },
    NetworkWhitelistGrant {
        entry: SageNetworkWhitelistEntry,

        #[serde(skip_serializing_if = "Option::is_none")]
        network_id: Option<String>,
    },
}

pub(crate) struct BridgeOrigin {
    pub app: SharedSageApp,
}

#[derive(Debug, Clone)]
pub(crate) struct PendingBridgeApproval {
    pub approval_id: String,
    pub app_id: String,
    pub registry_kind: BridgeRegistryKind,
    pub approval: RustBridgeApprovalRequest,
    pub request: RustBridgeRequest,
    pub created_at_ms: u64,
    pub expires_at_ms: u64,
    pub approved_fingerprint: Option<u32>,
    /// Whether the approval card should collect a password. A hint captured
    /// when the approval was queued, not the authority: the gate re-checks at
    /// resolve time, and a card that guessed wrong is told to ask.
    pub requires_password: bool,
    /// Incorrect password attempts spent so far. Counted on the host, so a
    /// reload of the approval webview cannot reset it.
    pub password_attempts: u8,
}

impl RustBridgeInvokeResult {
    pub fn success(id: &str, result: &Value) -> Self {
        RustBridgeInvokeResult::Success(RustBridgeSuccessResponse::new(id, result))
    }

    pub fn error(id: &str, code: &str, message: impl Into<String>) -> Self {
        RustBridgeInvokeResult::Error(RustBridgeErrorResponse::new(id, code, message))
    }

    pub fn pending() -> Self {
        RustBridgeInvokeResult::Pending
    }
}

impl TryFrom<RustBridgeInvokeResult> for RustBridgeResponse {
    type Error = String;

    fn try_from(value: RustBridgeInvokeResult) -> Result<Self, String> {
        match value {
            RustBridgeInvokeResult::Success(response) => Ok(Self::Success(response)),
            RustBridgeInvokeResult::Error(response) => Ok(Self::Error(response)),
            RustBridgeInvokeResult::Pending => Err("Invoke result is pending".to_string()),
        }
    }
}

impl From<RustBridgeResponse> for RustBridgeInvokeResult {
    fn from(response: RustBridgeResponse) -> Self {
        match response {
            RustBridgeResponse::Success(response) => RustBridgeInvokeResult::Success(response),
            RustBridgeResponse::Error(response) => RustBridgeInvokeResult::Error(response),
        }
    }
}

impl RustBridgeResponse {
    pub fn success(id: &str, result: &Value) -> Self {
        Self::Success(RustBridgeSuccessResponse::new(id, result))
    }

    pub fn error(id: &str, code: &str, message: impl Into<String>) -> Self {
        RustBridgeResponse::Error(RustBridgeErrorResponse::new(id, code, message))
    }
}

impl RustBridgeSuccessResponse {
    pub(crate) fn new(id: &str, result: &Value) -> Self {
        Self {
            bridge_version: "v1".into(),
            id: id.into(),
            ok: true,
            result_json: serde_json::to_string(result).unwrap_or_else(|_| "null".to_string()),
        }
    }
}

impl RustBridgeErrorResponse {
    pub(crate) fn new(id: &str, code: &str, message: impl Into<String>) -> Self {
        Self {
            bridge_version: "v1".into(),
            id: id.into(),
            ok: false,
            error: RustBridgeErrorPayload {
                code: code.into(),
                message: message.into(),
            },
        }
    }
}
