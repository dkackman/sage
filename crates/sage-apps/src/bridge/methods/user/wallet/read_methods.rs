use async_trait::async_trait;

use crate::bridge::capabilities::UserBridgeCapability;
use crate::bridge::methods::shared::BridgeMethodCapability;
use crate::bridge::methods::{BridgeContext, BridgeMethod, BridgeTools};
use crate::bridge::methods::user::app::encode_request_success;
use crate::bridge::{
    failure, RustBridgeApprovalRequest, RustBridgeRequest, RustBridgeResponse,
};

use sage_api::{CheckAddress, GetCoins, GetCoinsByIds, GetDerivations, GetKey, GetKeys, GetPendingTransactions, GetSpendableCoinCount, GetSyncStatus, GetTransaction, GetTransactions, GetVersion};

macro_rules! define_wallet_read_no_params_async_method {
    ($struct_name:ident, $capability:ident, $method_name:expr, $request_ident:ident, $result_label:expr, $handler:ident) => {
        #[derive(Debug, Clone, Copy)]
        pub struct $struct_name;

        #[async_trait]
        impl BridgeMethod for $struct_name {
            fn capability(&self) -> BridgeMethodCapability {
                BridgeMethodCapability::user(UserBridgeCapability::$capability)
            }

            fn approval_request(&self, _ctx: BridgeContext<'_>, _request: &RustBridgeRequest) -> Option<RustBridgeApprovalRequest> {
                None
            }

            async fn handle(&self, _ctx: BridgeContext<'_>, tools: BridgeTools<'_>, request: &RustBridgeRequest) -> RustBridgeResponse {
                let sage = tools.app_state.lock().await;

                match sage.$handler($request_ident {}).await {
                    Ok(result) => encode_request_success(request, result, $result_label),
                    Err(err) => failure(&request.channel, &request.id, "internal_error", format!("failed to execute {}: {err}", $method_name)),
                }
            }
        }
    };
}

macro_rules! define_wallet_read_no_params_sync_method {
    ($struct_name:ident, $capability:ident, $method_name:expr, $request_ident:ident, $result_label:expr, $handler:ident) => {
        #[derive(Debug, Clone, Copy)]
        pub struct $struct_name;

        #[async_trait]
        impl BridgeMethod for $struct_name {
            fn capability(&self) -> BridgeMethodCapability {
                BridgeMethodCapability::user(UserBridgeCapability::$capability)
            }

            fn approval_request(&self, _ctx: BridgeContext<'_>, _request: &RustBridgeRequest) -> Option<RustBridgeApprovalRequest> {
                None
            }

            async fn handle(&self, _ctx: BridgeContext<'_>, tools: BridgeTools<'_>, request: &RustBridgeRequest) -> RustBridgeResponse {
                let sage = tools.app_state.lock().await;

                match sage.$handler($request_ident {}) {
                    Ok(result) => encode_request_success(request, result, $result_label),
                    Err(err) => failure(&request.channel, &request.id, "internal_error", format!("failed to execute {}: {err}", $method_name)),
                }
            }
        }
    };
}

macro_rules! define_wallet_read_params_async_method {
    ($struct_name:ident, $capability:ident, $method_name:expr, $request_ty:ty, $result_label:expr, $handler:ident) => {
        #[derive(Debug, Clone, Copy)]
        pub struct $struct_name;

        #[async_trait]
        impl BridgeMethod for $struct_name {
            fn capability(&self) -> BridgeMethodCapability {
                BridgeMethodCapability::user(UserBridgeCapability::$capability)
            }

            fn approval_request(&self, _ctx: BridgeContext<'_>, _request: &RustBridgeRequest) -> Option<RustBridgeApprovalRequest> {
                None
            }

            async fn handle(&self, _ctx: BridgeContext<'_>, tools: BridgeTools<'_>, request: &RustBridgeRequest) -> RustBridgeResponse {
                let Some(params_json) = request.params_json.as_deref() else {
                    return failure(&request.channel, &request.id, "invalid_request", format!("{} requires params", $method_name));
                };

                let params: $request_ty = match serde_json::from_str(params_json) {
                    Ok(params) => params,
                    Err(err) => {
                        return failure(&request.channel, &request.id, "invalid_request", format!("Failed to decode {} params: {err}", $method_name));
                    }
                };

                let sage = tools.app_state.lock().await;

                match sage.$handler(params).await {
                    Ok(result) => encode_request_success(request, result, $result_label),
                    Err(err) => failure(&request.channel, &request.id, "internal_error", format!("failed to execute {}: {err}", $method_name)),
                }
            }
        }
    };
}

macro_rules! define_wallet_read_params_sync_method {
    ($struct_name:ident, $capability:ident, $method_name:expr, $request_ty:ty, $result_label:expr, $handler:ident) => {
        #[derive(Debug, Clone, Copy)]
        pub struct $struct_name;

        #[async_trait]
        impl BridgeMethod for $struct_name {
            fn capability(&self) -> BridgeMethodCapability {
                BridgeMethodCapability::user(UserBridgeCapability::$capability)
            }

            fn approval_request(&self, _ctx: BridgeContext<'_>, _request: &RustBridgeRequest) -> Option<RustBridgeApprovalRequest> {
                None
            }

            async fn handle(&self, _ctx: BridgeContext<'_>, tools: BridgeTools<'_>, request: &RustBridgeRequest) -> RustBridgeResponse {
                let Some(params_json) = request.params_json.as_deref() else {
                    return failure(&request.channel, &request.id, "invalid_request", format!("{} requires params", $method_name));
                };

                let params: $request_ty = match serde_json::from_str(params_json) {
                    Ok(params) => params,
                    Err(err) => {
                        return failure(&request.channel, &request.id, "invalid_request", format!("Failed to decode {} params: {err}", $method_name));
                    }
                };

                let sage = tools.app_state.lock().await;

                match sage.$handler(params) {
                    Ok(result) => encode_request_success(request, result, $result_label),
                    Err(err) => failure(&request.channel, &request.id, "internal_error", format!("failed to execute {}: {err}", $method_name)),
                }
            }
        }
    };
}


define_wallet_read_no_params_sync_method!(
    WalletGetKeys,
    WalletGetKeys,
    "wallet.getKeys",
    GetKeys,
    "wallet.getKeys result",
    get_keys
);

define_wallet_read_params_sync_method!(
    WalletGetKey,
    WalletGetKey,
    "wallet.getKey",
    GetKey,
    "wallet.getKey result",
    get_key
);
define_wallet_read_no_params_async_method!(WalletGetSyncStatus, WalletGetSyncStatus, "wallet.getSyncStatus", GetSyncStatus, "wallet.getSyncStatus result", get_sync_status);
define_wallet_read_no_params_sync_method!(WalletGetVersion, WalletGetVersion, "wallet.getVersion", GetVersion, "wallet.getVersion result", get_version);
define_wallet_read_no_params_async_method!(WalletGetPendingTransactions, WalletGetPendingTransactions, "wallet.getPendingTransactions", GetPendingTransactions, "wallet.getPendingTransactions result", get_pending_transactions);

define_wallet_read_params_async_method!(WalletCheckAddress, WalletCheckAddress, "wallet.checkAddress", CheckAddress, "wallet.checkAddress result", check_address);
define_wallet_read_params_async_method!(WalletGetDerivations, WalletGetDerivations, "wallet.getDerivations", GetDerivations, "wallet.getDerivations result", get_derivations);
define_wallet_read_params_async_method!(WalletGetSpendableCoinCount, WalletGetSpendableCoinCount, "wallet.getSpendableCoinCount", GetSpendableCoinCount, "wallet.getSpendableCoinCount result", get_spendable_coin_count);
define_wallet_read_params_async_method!(WalletGetCoinsByIds, WalletGetCoinsByIds, "wallet.getCoinsByIds", GetCoinsByIds, "wallet.getCoinsByIds result", get_coins_by_ids);
define_wallet_read_params_async_method!(WalletGetCoins, WalletGetCoins, "wallet.getCoins", GetCoins, "wallet.getCoins result", get_coins);
define_wallet_read_params_async_method!(WalletGetTransaction, WalletGetTransaction, "wallet.getTransaction", GetTransaction, "wallet.getTransaction result", get_transaction);
define_wallet_read_params_async_method!(WalletGetTransactions, WalletGetTransactions, "wallet.getTransactions", GetTransactions, "wallet.getTransactions result", get_transactions);
