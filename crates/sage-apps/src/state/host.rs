use crate::bridge::BridgeState;
use crate::runtime::AppRuntimeState;
use crate::sandbox::SandboxStateStore;

#[derive(Debug, Default)]
pub struct AppsHostState {
    pub runtime: AppRuntimeState,
    pub bridge: BridgeState,
    pub sandbox: SandboxStateStore,
}
