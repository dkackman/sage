use std::collections::HashMap;

use super::methods::system::{
    AppGetInfo, BridgePing, BridgeSend, SageGetCapabilities, SageRequestCapabilityGrant,
    SageRequestNetworkWhitelistGrant,
};
use super::methods::wallet::WalletSendXch;
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

        match kind {
            BridgeRegistryKind::User => {}
            BridgeRegistryKind::System => {
                // future system-only methods go here
            }
        }

        Self { methods }
    }

    pub fn get(&self, method: &str) -> Option<&dyn BridgeMethod> {
        self.methods.get(method).map(|m| m.as_ref())
    }
}

impl std::fmt::Debug for BridgeRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BridgeRegistry")
            .field("method_count", &self.methods.len())
            .finish()
    }
}
