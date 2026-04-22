use std::collections::HashMap;

use super::methods::user::{
    AppGetInfo, BridgePing, BridgeSend, SageGetCapabilities,
    SageRequestCapabilityGrant, SageRequestNetworkWhitelistGrant, WalletSendXch,
};
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
    methods.insert("sage.getCapabilities", Box::new(SageGetCapabilities));
    methods.insert(
        "sage.requestCapabilityGrant",
        Box::new(SageRequestCapabilityGrant),
    );
    methods.insert(
        "sage.requestNetworkWhitelistGrant",
        Box::new(SageRequestNetworkWhitelistGrant),
    );
    methods.insert(
        "sage.requestNetwortWhitelistGrant",
        Box::new(SageRequestNetworkWhitelistGrant),
    );
    methods.insert("wallet.sendXch", Box::new(WalletSendXch));

    methods
}

fn build_system_methods() -> HashMap<&'static str, Box<dyn BridgeMethod>> {
    let mut methods = build_user_methods();

    // Register system-only methods here later, for example:
    // methods.insert("system.taskManager.list", Box::new(SystemTaskManagerList));

    methods
}

impl std::fmt::Debug for BridgeRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BridgeRegistry")
            .field("method_count", &self.methods.len())
            .finish()
    }
}
