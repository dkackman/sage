use serde::{Deserialize, Serialize};
use specta::Type;
use crate::bridge::capabilities::UserBridgeCapability;
use crate::lifecycle::{GrantedCapabilitiesChange, GrantedNetworkWhitelistChange};
use crate::types::SageNetworkPermissionTarget;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GrantedCapabilitiesChangeEvent {
    pub channel: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub removed_granted_capabilities: Vec<UserBridgeCapability>,
    pub added_granted_capabilities: Vec<UserBridgeCapability>,
    pub full_granted_capabilities: Vec<UserBridgeCapability>,
}

impl GrantedCapabilitiesChangeEvent {
    pub fn from_change(channel: String, change: GrantedCapabilitiesChange) -> Self {
        Self {
            channel,
            event_type: "grantedCapabilitiesChange".to_string(),
            removed_granted_capabilities: change.removed,
            added_granted_capabilities: change.added,
            full_granted_capabilities: change.full,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GrantedNetworkWhitelistChangeEvent {
    pub channel: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub removed_granted_network_whitelist: Vec<SageNetworkPermissionTarget>,
    pub added_granted_network_whitelist: Vec<SageNetworkPermissionTarget>,
    pub full_granted_network_whitelist: Vec<SageNetworkPermissionTarget>,
}

impl GrantedNetworkWhitelistChangeEvent {
    pub fn from_change(channel: String, change: GrantedNetworkWhitelistChange) -> Self {
        Self {
            channel,
            event_type: "grantedNetworkWhitelistChange".to_string(),
            removed_granted_network_whitelist: change.removed,
            added_granted_network_whitelist: change.added,
            full_granted_network_whitelist: change.full,
        }
    }
}
