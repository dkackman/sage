use std::collections::BTreeMap;

use serde::Serialize;
use specta::Type;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SageAppRuntimeRecord {
    pub runtime_id: String,
    pub app_id: String,
    pub app_name: String,
    pub entry_src: String,
    pub webview_label: String,
    pub host_window_label: String,
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

#[derive(Debug, Default)]
pub struct AppRuntimeState {
    pub by_runtime_id: Mutex<BTreeMap<String, SageAppRuntimeRecord>>,
    pub runtime_by_app_id: Mutex<BTreeMap<String, String>>,
}

pub fn runtime_id_for(app_id: &str) -> String {
    format!("runtime-{app_id}")
}

pub fn inline_label_for(app_id: &str) -> String {
    format!("app-inline-{app_id}")
}
