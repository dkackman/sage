use crate::apps::bridge::BridgeState;
use crate::apps::runtime::AppRuntimeState;
use crate::apps::sandbox::SandboxStateStore;

#[derive(Debug, Default)]
pub struct AppsHostState {
    pub runtime: AppRuntimeState,
    pub bridge: BridgeState,
    pub sandbox: SandboxStateStore,
}
