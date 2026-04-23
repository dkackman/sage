use std::collections::HashMap;
use crate::bridge::methods::user::app::SageRequestNetworkWhitelistGrant;
use super::methods::user::{AppGetInfo, BridgePing, BridgeSend, SageAppLifecycleReadyToStop, SageAppLifecycleSetBeforeStopListener, SageGetCapabilities, SageRequestCapabilityGrant, WalletSendXch};
use super::methods::BridgeMethod;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeRegistryKind {
    User,
    System,
}

pub struct BridgeRegistry {
    methods: HashMap<&'static str, Box<dyn BridgeMethod>>,
}

impl BridgeRegistry {
    pub fn new(kind: BridgeRegistryKind) -> Self {
        match kind {
            BridgeRegistryKind::User => Self {
                methods: build_user_methods(),
            },
            BridgeRegistryKind::System => Self {
                methods: build_system_methods(),
            },
        }
    }

    pub fn get(&self, method: &str) -> Option<&dyn BridgeMethod> {
        self.methods.get(method).map(|m| m.as_ref())
    }
}

fn build_user_methods() -> HashMap<&'static str, Box<dyn BridgeMethod>> {
    let mut methods: HashMap<&'static str, Box<dyn BridgeMethod>> = HashMap::new();

    methods.insert("bridge.ping", Box::new(BridgePing));
    methods.insert("bridge.send", Box::new(BridgeSend));
    methods.insert("app.getInfo", Box::new(AppGetInfo));
    methods.insert("app.getCapabilities", Box::new(SageGetCapabilities));
    methods.insert(
        "app.requestCapabilityGrant",
        Box::new(SageRequestCapabilityGrant),
    );
    methods.insert(
        "app.requestNetworkWhitelistGrant",
        Box::new(SageRequestNetworkWhitelistGrant),
    );
    methods.insert(
        "app.lifecycle.setBeforeStopListener",
        Box::new(SageAppLifecycleSetBeforeStopListener),
    );
    methods.insert(
        "app.lifecycle.readyToStop",
        Box::new(SageAppLifecycleReadyToStop),
    );
    methods.insert("wallet.sendXch", Box::new(WalletSendXch));

    methods
}

fn build_system_methods() -> HashMap<&'static str, Box<dyn BridgeMethod>> {
    let mut methods = build_user_methods();

    methods.insert("runtimeManager.listRuntimes", Box::new(super::methods::system::runtime_manager::SystemListRuntimes));
    methods.insert("runtimeManager.focusRuntime", Box::new(super::methods::system::runtime_manager::SystemFocusRuntime));
    methods.insert("runtimeManager.killRuntime", Box::new(super::methods::system::runtime_manager::SystemKillRuntime));
    methods.insert("runtimeManager.hideRuntime", Box::new(super::methods::system::runtime_manager::SystemHideRuntime));

    methods
}

impl std::fmt::Debug for BridgeRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BridgeRegistry")
            .field("method_count", &self.methods.len())
            .finish()
    }
}
