use std::collections::HashMap;
use crate::types::SageApp;
use super::methods::BridgeMethod;
use super::methods::system::runtime_manager::{
    RuntimeManagerFocusRuntime, RuntimeManagerHideRuntime,
    RuntimeManagerKillRuntime, RuntimeManagerListRuntimes,
};
use super::methods::user::{
    AppGetCapabilities, AppGetInfo, AppLifecycleReadyToStop,
    AppLifecycleSetBeforeStopListener, AppRequestCapabilityGrant,
    AppRequestNetworkWhitelistGrant, BridgePing, BridgeSend, WalletCheckAddress,
    WalletGetCoins, WalletGetCoinsByIds, WalletGetDerivations, WalletGetKey,
    WalletGetKeys, WalletGetPendingTransactions, WalletGetSecretKey,
    WalletGetSpendableCoinCount, WalletGetSyncStatus, WalletGetTransaction,
    WalletGetTransactions, WalletGetVersion, WalletSendXch,
};

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

    pub fn new_for_app(app: &SageApp) -> Self {
        match app {
            SageApp::User(_) => Self::new(BridgeRegistryKind::User),
            SageApp::System(_) => Self::new(BridgeRegistryKind::System),
        }
    }

    pub fn get(&self, method: &str) -> Option<&dyn BridgeMethod> {
        self.methods.get(method).map(|method| method.as_ref())
    }

    pub fn iter(&self) -> impl Iterator<Item = (&'static str, &dyn BridgeMethod)> {
        self.methods
            .iter()
            .map(|(name, method)| (*name, method.as_ref()))
    }
}

fn build_user_methods() -> HashMap<&'static str, Box<dyn BridgeMethod>> {
    let mut methods: HashMap<&'static str, Box<dyn BridgeMethod>> = HashMap::new();

    // Bridge
    methods.insert("bridge.ping", Box::new(BridgePing));
    methods.insert("bridge.send", Box::new(BridgeSend));

    // App
    methods.insert("app.getInfo", Box::new(AppGetInfo));
    methods.insert("app.getCapabilities", Box::new(AppGetCapabilities));
    methods.insert("app.requestCapabilityGrant", Box::new(AppRequestCapabilityGrant));
    methods.insert(
        "app.requestNetworkWhitelistGrant",
        Box::new(AppRequestNetworkWhitelistGrant),
    );
    methods.insert(
        "app.lifecycle.setBeforeStopListener",
        Box::new(AppLifecycleSetBeforeStopListener),
    );
    methods.insert("app.lifecycle.readyToStop", Box::new(AppLifecycleReadyToStop));

    // Wallet keys / secrets
    methods.insert("wallet.getKeys", Box::new(WalletGetKeys));
    methods.insert("wallet.getKey", Box::new(WalletGetKey));
    methods.insert("wallet.getSecretKey", Box::new(WalletGetSecretKey));

    // Wallet XCH
    methods.insert("wallet.sendXch", Box::new(WalletSendXch));

    // Wallet read/query
    methods.insert("wallet.getSyncStatus", Box::new(WalletGetSyncStatus));
    methods.insert("wallet.getVersion", Box::new(WalletGetVersion));
    methods.insert(
        "wallet.getPendingTransactions",
        Box::new(WalletGetPendingTransactions),
    );
    methods.insert("wallet.checkAddress", Box::new(WalletCheckAddress));
    methods.insert("wallet.getDerivations", Box::new(WalletGetDerivations));
    methods.insert(
        "wallet.getSpendableCoinCount",
        Box::new(WalletGetSpendableCoinCount),
    );
    methods.insert("wallet.getCoinsByIds", Box::new(WalletGetCoinsByIds));
    methods.insert("wallet.getCoins", Box::new(WalletGetCoins));
    methods.insert("wallet.getTransaction", Box::new(WalletGetTransaction));
    methods.insert("wallet.getTransactions", Box::new(WalletGetTransactions));

    methods
}

fn build_system_methods() -> HashMap<&'static str, Box<dyn BridgeMethod>> {
    let mut methods = build_user_methods();

    methods.insert(
        "runtimeManager.listRuntimes",
        Box::new(RuntimeManagerListRuntimes),
    );
    methods.insert(
        "runtimeManager.focusRuntime",
        Box::new(RuntimeManagerFocusRuntime),
    );
    methods.insert("runtimeManager.hideRuntime", Box::new(RuntimeManagerHideRuntime));
    methods.insert("runtimeManager.killRuntime", Box::new(RuntimeManagerKillRuntime));

    methods
}

impl std::fmt::Debug for BridgeRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BridgeRegistry")
            .field("method_count", &self.methods.len())
            .finish()
    }
}
