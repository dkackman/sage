use specta::TypeCollection;
use specta_typescript::{BigIntExportBehavior, Typescript};
use sage_api::{GetKey, GetKeyResponse, GetKeys, GetKeysResponse, GetSecretKey, GetSecretKeyResponse, TransactionResponse};
use crate::bridge::methods::system::runtime_manager::RuntimeTargetParams;
use crate::bridge::methods::system::RuntimeManagerRuntimesChangedEvent;
use crate::bridge::methods::user::app::get_info::{AppGetInfoResult, SageNetworkPermissionInfo};
use crate::bridge::methods::user::app::{GrantedCapabilitiesChangeEvent, GrantedNetworkWhitelistChangeEvent};
use crate::bridge::methods::user::app::request_capability_grant::{RequestCapabilityGrantParams, RequestCapabilityGrantResult};
use crate::bridge::methods::user::app::request_network_whitelist_grant::{RequestNetworkWhitelistGrantParams, RequestNetworkWhitelistGrantResult};
use crate::bridge::methods::user::bridge::ping::BridgePingResult;
use crate::bridge::methods::user::bridge::send::BridgeSendResult;
use crate::bridge::methods::user::wallet::send_xch::WalletSendXchParams;
use crate::runtime::{ReadyToStopParams, RuntimeAckResult, SageAppRuntimeRecord, SageLifecycleBeforeStopDetail, SetBeforeStopListenerParams, SystemKillRuntimeResult};

pub fn export_system_bridge_typescript() -> Result<String, String> {
    let mut types = TypeCollection::default();

    types.register::<RuntimeTargetParams>();
    types.register::<SageAppRuntimeRecord>();
    types.register::<SystemKillRuntimeResult>();
    types.register::<RuntimeManagerRuntimesChangedEvent>();

    Typescript::default()
        .bigint(BigIntExportBehavior::Number)
        .export(&types)
        .map_err(|err| format!("failed to export system bridge TS types: {err}"))
}

pub fn export_user_bridge_typescript() -> Result<String, String> {
    let mut types = TypeCollection::default();

    types.register::<BridgePingResult>();
    types.register::<BridgeSendResult>();
    types.register::<SageNetworkPermissionInfo>();
    types.register::<AppGetInfoResult>();
    types.register::<WalletSendXchParams>();
    types.register::<TransactionResponse>();
    types.register::<RequestCapabilityGrantParams>();
    types.register::<RequestCapabilityGrantResult>();
    types.register::<RequestNetworkWhitelistGrantParams>();
    types.register::<RequestNetworkWhitelistGrantResult>();
    types.register::<GrantedCapabilitiesChangeEvent>();
    types.register::<GrantedNetworkWhitelistChangeEvent>();
    types.register::<SageLifecycleBeforeStopDetail>();
    types.register::<SetBeforeStopListenerParams>();
    types.register::<ReadyToStopParams>();
    types.register::<RuntimeAckResult>();
    types.register::<GetKeys>();
    types.register::<GetKeysResponse>();
    types.register::<GetKey>();
    types.register::<GetKeyResponse>();
    types.register::<GetSecretKey>();
    types.register::<GetSecretKeyResponse>();

    Typescript::default()
        .bigint(BigIntExportBehavior::Number)
        .export(&types)
        .map_err(|err| format!("failed to export user bridge TS types: {err}"))
}
