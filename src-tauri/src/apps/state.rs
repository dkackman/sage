use crate::apps::bridge::BridgeState;
use crate::apps::runtime::AppRuntimeState;
use crate::apps::sandbox::SandboxStateStore;

#[derive(Default)]
pub struct AppsHostState {
    pub runtime: AppRuntimeState,
    pub bridge: BridgeState,
    pub sandbox: SandboxStateStore,
}
