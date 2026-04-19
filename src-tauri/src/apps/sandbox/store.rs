use std::collections::HashMap;

use tokio::sync::Mutex;

use super::types::{
    SandboxIsolationProbeResult, SandboxNetworkProbeResult,
    SandboxPersistenceReadProbeResult, SandboxPersistenceWriteProbeResult,
    SandboxRunState, SandboxState, SandboxStorageClearProbeResult,
    build_initial_sandbox_state,
};

#[derive(Debug, Clone)]
pub(crate) struct SandboxAppResult<T> {
    pub app_id: String,
    pub data: T,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SandboxRunResults {
    pub isolation: Vec<SandboxAppResult<SandboxIsolationProbeResult>>,
    pub persistence_write: Vec<SandboxAppResult<SandboxPersistenceWriteProbeResult>>,
    pub persistence_read: Vec<SandboxAppResult<SandboxPersistenceReadProbeResult>>,
    pub clear_cycle: Vec<SandboxAppResult<SandboxStorageClearProbeResult>>,
    pub network: Vec<SandboxAppResult<SandboxNetworkProbeResult>>,
}

pub struct SandboxStateStore {
    pub baseline: Mutex<SandboxState>,
    pub current_run: Mutex<Option<SandboxRunState>>,
    pub(crate) runs: Mutex<HashMap<String, SandboxRunResults>>,
    pub(crate) running: Mutex<bool>,
}

impl Default for SandboxStateStore {
    fn default() -> Self {
        Self {
            baseline: Mutex::new(build_initial_sandbox_state()),
            current_run: Mutex::new(None),
            runs: Mutex::new(HashMap::new()),
            running: Mutex::new(false),
        }
    }
}

pub(crate) fn replace_by_app_id<T: Clone>(
    items: &mut Vec<SandboxAppResult<T>>,
    next: SandboxAppResult<T>,
) {
    items.retain(|item| item.app_id != next.app_id);
    items.push(next);
}
