use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use specta::Type;
use tokio::sync::{oneshot, Mutex};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SageAppRuntimeKind {
    User,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SageLifecycleBeforeStopDetail {
    pub request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SetBeforeStopListenerParams {
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReadyToStopParams {
    pub request_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAckResult {
    pub ok: bool,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SageAppRuntimeRecord {
    pub runtime_id: String,
    pub app_id: String,
    pub app_name: String,
    pub entry_src: String,
    pub webview_label: String,
    pub host_window_label: String,
    pub runtime_kind: SageAppRuntimeKind,
    pub mode: String,
    pub state: String,
    pub started_at: i64,
    pub last_active_at: i64,
    pub visible: bool,
    pub internal: bool,
    pub active_batch_count: u32,
    pub active_socket_count: u32,
    pub in_flight_request_count: u32,
}

#[derive(Default)]
pub struct AppRuntimeState {
    pub by_runtime_id: Mutex<BTreeMap<String, SageAppRuntimeRecord>>,
    pub runtime_by_app_id: Mutex<BTreeMap<String, String>>,
    pub before_stop_listeners_by_app_id: Mutex<BTreeSet<String>>,
    pub pending_stop_ready: Mutex<BTreeMap<String, oneshot::Sender<()>>>,
}

impl std::fmt::Debug for AppRuntimeState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppRuntimeState").finish()
    }
}

pub fn runtime_id_for(app_id: &str, runtime_kind: SageAppRuntimeKind) -> String {
    match runtime_kind {
        SageAppRuntimeKind::User => format!("runtime-{app_id}"),
        SageAppRuntimeKind::System => format!("system-runtime-{app_id}"),
    }
}

pub fn inline_label_for(app_id: &str, runtime_kind: SageAppRuntimeKind) -> String {
    match runtime_kind {
        SageAppRuntimeKind::User => format!("app-inline-{app_id}"),
        SageAppRuntimeKind::System => format!("system-app-inline-{app_id}"),
    }
}
