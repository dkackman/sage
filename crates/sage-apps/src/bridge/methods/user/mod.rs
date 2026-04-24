pub mod app;
pub mod bridge;
pub mod wallet;

pub use app::{
    AppGetInfo,
    AppLifecycleReadyToStop,
    AppLifecycleSetBeforeStopListener,
    AppGetCapabilities,
    AppRequestCapabilityGrant,
    AppRequestNetworkWhitelistGrant,
};

pub use bridge::{
    BridgePing,
    BridgeSend,
};

pub use wallet::{
    WalletGetSecretKey,

    WalletSendXch,

    WalletGetKey,
    WalletGetKeys,
    WalletGetSyncStatus,
    WalletGetVersion,
    WalletGetPendingTransactions,
    WalletCheckAddress,
    WalletGetDerivations,
    WalletGetSpendableCoinCount,
    WalletGetCoinsByIds,
    WalletGetCoins,
    WalletGetTransaction,
    WalletGetTransactions,
};