use crate::bridge::types::BridgeState;
use crate::runtime::state::types::AppRuntimeState;
use crate::sandbox::SandboxStateStore;

#[derive(Debug, Default)]
pub struct AppsHostState {
    pub runtime: AppRuntimeState,
    pub bridge: BridgeState,
    pub sandbox: SandboxStateStore,
}
