use std::collections::HashMap;

use super::methods::system::{AppGetInfo, BridgePing, BridgeSend, SageGetCapabilities};
use super::methods::wallet::WalletSendXch;
use super::methods::BridgeMethod;

pub struct BridgeRegistry {
    methods: HashMap<&'static str, Box<dyn BridgeMethod>>,
}

impl BridgeRegistry {
    pub fn new() -> Self {
        let mut methods: HashMap<&'static str, Box<dyn BridgeMethod>> = HashMap::new();

        methods.insert("bridge.ping", Box::new(BridgePing));
        methods.insert("bridge.send", Box::new(BridgeSend));
        methods.insert("app.getInfo", Box::new(AppGetInfo));
        methods.insert("sage.getCapabilities", Box::new(SageGetCapabilities));
        methods.insert("wallet.sendXch", Box::new(WalletSendXch));

        Self { methods }
    }

    pub fn get(&self, method: &str) -> Option<&dyn BridgeMethod> {
        self.methods.get(method).map(|m| m.as_ref())
    }
}
