use crate::bridge::capabilities::UserBridgeCapability;
use crate::types::SageNetworkPermissionTarget;

#[derive(Debug, Clone)]
pub struct GrantedCapabilitiesChange {
    pub removed: Vec<UserBridgeCapability>,
    pub added: Vec<UserBridgeCapability>,
    pub full: Vec<UserBridgeCapability>,
}

#[derive(Debug, Clone)]
pub struct GrantedNetworkWhitelistChange {
    pub removed: Vec<SageNetworkPermissionTarget>,
    pub added: Vec<SageNetworkPermissionTarget>,
    pub full: Vec<SageNetworkPermissionTarget>,
}

#[derive(Debug, Clone)]
pub enum GrantCapabilityOutcome {
    AlreadyGranted {
        capability: UserBridgeCapability,
        full_granted_capabilities: Vec<UserBridgeCapability>,
    },
    Granted {
        capability: UserBridgeCapability,
        change: GrantedCapabilitiesChange,
    },
}

#[derive(Debug, Clone)]
pub enum GrantNetworkWhitelistOutcome {
    AlreadyGranted {
        entry: SageNetworkPermissionTarget,
        full_granted_network_whitelist: Vec<SageNetworkPermissionTarget>,
    },
    Granted {
        entry: SageNetworkPermissionTarget,
        change: GrantedNetworkWhitelistChange,
    },
}
