use std::collections::BTreeMap;

use crate::bridge::capabilities::{SystemBridgeCapability, UserBridgeCapability};
use crate::types::{SageAppCapabilityDefinitionView, SageAppCapabilityFlagsView};

#[derive(Debug, Clone, Copy)]
pub struct CapabilityFlags {
    pub externally_observable: bool,
    pub accesses_sensitive_secret: bool,
    pub requestable_by_app: bool,
    pub user_grantable: bool,
    pub shared_with_app: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct CapabilityDefinition<C> {
    pub capability: C,
    pub label: &'static str,
    pub description: &'static str,
    pub flags: CapabilityFlags,
}

pub type UserCapabilityDefinition = CapabilityDefinition<UserBridgeCapability>;
pub type SystemCapabilityDefinition = CapabilityDefinition<SystemBridgeCapability>;

pub fn get_user_capability_definition(
    capability: UserBridgeCapability,
) -> Option<UserCapabilityDefinition> {
    Some(match capability {
        UserBridgeCapability::PersistentStorage => UserCapabilityDefinition {
            capability,
            label: "Persistent storage",
            description: "Allows the app to store data on this device between sessions.",
            flags: CapabilityFlags {
                externally_observable: false,
                accesses_sensitive_secret: false,
                requestable_by_app: true,
                user_grantable: true,
                shared_with_app: false,
            },
        },

        UserBridgeCapability::BridgeSend => UserCapabilityDefinition {
            capability,
            label: "Bridge messaging",
            description: "Allows the app to send messages through the Sage bridge.",
            flags: CapabilityFlags {
                externally_observable: false,
                accesses_sensitive_secret: false,
                requestable_by_app: true,
                user_grantable: false,
                shared_with_app: true,
            },
        },

        UserBridgeCapability::AppGetCapabilities => UserCapabilityDefinition {
            capability,
            label: "Read granted capabilities",
            description: "Allows the app to read the capabilities currently visible to it.",
            flags: CapabilityFlags {
                externally_observable: false,
                accesses_sensitive_secret: false,
                requestable_by_app: true,
                user_grantable: false,
                shared_with_app: true,
            },
        },

        UserBridgeCapability::AppGetInfo => UserCapabilityDefinition {
            capability,
            label: "Read app information",
            description: "Allows the app to read its Sage app identity and permission information.",
            flags: CapabilityFlags {
                externally_observable: false,
                accesses_sensitive_secret: false,
                requestable_by_app: true,
                user_grantable: false,
                shared_with_app: true,
            },
        },

        UserBridgeCapability::AppLifecycleReadyToStop => UserCapabilityDefinition {
            capability,
            label: "Acknowledge app shutdown",
            description: "Allows the app to acknowledge that it is ready to stop after a lifecycle request.",
            flags: CapabilityFlags {
                externally_observable: false,
                accesses_sensitive_secret: false,
                requestable_by_app: true,
                user_grantable: false,
                shared_with_app: true,
            },
        },

        UserBridgeCapability::AppLifecycleSetBeforeStopListener => UserCapabilityDefinition {
            capability,
            label: "Listen before app shutdown",
            description: "Allows the app to register a before-stop lifecycle listener.",
            flags: CapabilityFlags {
                externally_observable: false,
                accesses_sensitive_secret: false,
                requestable_by_app: true,
                user_grantable: false,
                shared_with_app: true,
            },
        },

        UserBridgeCapability::AppRequestCapabilityGrant => UserCapabilityDefinition {
            capability,
            label: "Request additional capability",
            description: "Allows the app to request a capability grant after installation.",
            flags: CapabilityFlags {
                externally_observable: false,
                accesses_sensitive_secret: false,
                requestable_by_app: true,
                user_grantable: false,
                shared_with_app: true,
            },
        },

        UserBridgeCapability::AppRequestNetworkWhitelistGrant => UserCapabilityDefinition {
            capability,
            label: "Request network access",
            description: "Allows the app to request access to an additional network target after installation.",
            flags: CapabilityFlags {
                externally_observable: false,
                accesses_sensitive_secret: false,
                requestable_by_app: true,
                user_grantable: false,
                shared_with_app: true,
            },
        },

        UserBridgeCapability::WalletSendXch => UserCapabilityDefinition {
            capability,
            label: "Send XCH",
            description: "Allows the app to request XCH transactions from your wallet.",
            flags: CapabilityFlags {
                externally_observable: true,
                accesses_sensitive_secret: false,
                requestable_by_app: true,
                user_grantable: true,
                shared_with_app: true,
            },
        },

        UserBridgeCapability::WalletSendXchAutoSubmit => UserCapabilityDefinition {
            capability,
            label: "Automatic XCH send",
            description: "Allows the app to submit XCH transactions without asking for per-transaction approval.",
            flags: CapabilityFlags {
                externally_observable: false,
                accesses_sensitive_secret: false,
                requestable_by_app: false,
                user_grantable: false,
                shared_with_app: false,
            },
        },
    })
}

pub fn get_system_capability_definition(
    capability: SystemBridgeCapability,
) -> Option<SystemCapabilityDefinition> {
    Some(match capability {
        SystemBridgeCapability::RuntimeManagerListRuntimes => SystemCapabilityDefinition {
            capability,
            label: "List app runtimes",
            description: "Allows the system app to inspect running Sage app runtimes.",
            flags: CapabilityFlags {
                externally_observable: false,
                accesses_sensitive_secret: false,
                requestable_by_app: true,
                user_grantable: false,
                shared_with_app: true,
            },
        },

        SystemBridgeCapability::RuntimeManagerFocusRuntime => SystemCapabilityDefinition {
            capability,
            label: "Focus app runtimes",
            description: "Allows the system app to focus running Sage app runtimes.",
            flags: CapabilityFlags {
                externally_observable: false,
                accesses_sensitive_secret: false,
                requestable_by_app: true,
                user_grantable: false,
                shared_with_app: true,
            },
        },

        SystemBridgeCapability::RuntimeManagerHideRuntime => SystemCapabilityDefinition {
            capability,
            label: "Hide app runtimes",
            description: "Allows the system app to hide running Sage app runtimes.",
            flags: CapabilityFlags {
                externally_observable: false,
                accesses_sensitive_secret: false,
                requestable_by_app: true,
                user_grantable: false,
                shared_with_app: true,
            },
        },

        SystemBridgeCapability::RuntimeManagerKillRuntime => SystemCapabilityDefinition {
            capability,
            label: "Kill app runtimes",
            description: "Allows the system app to stop running Sage app runtimes.",
            flags: CapabilityFlags {
                externally_observable: false,
                accesses_sensitive_secret: false,
                requestable_by_app: true,
                user_grantable: false,
                shared_with_app: true,
            },
        },

        SystemBridgeCapability::RuntimeManagerListenRuntimesChanged => {
            SystemCapabilityDefinition {
                capability,
                label: "Observe runtime changes",
                description: "Allows the system app to receive events when Sage app runtimes change.",
                flags: CapabilityFlags {
                    externally_observable: false,
                    accesses_sensitive_secret: false,
                    requestable_by_app: true,
                    user_grantable: false,
                    shared_with_app: true,
                },
            }
        }
    })
}

pub fn require_user_capability_definition(
    capability: UserBridgeCapability,
) -> anyhow::Result<UserCapabilityDefinition> {
    get_user_capability_definition(capability).ok_or_else(|| {
        anyhow::anyhow!("unknown user capability: {}", capability.key())
    })
}

pub fn require_system_capability_definition(
    capability: SystemBridgeCapability,
) -> anyhow::Result<SystemCapabilityDefinition> {
    get_system_capability_definition(capability).ok_or_else(|| {
        anyhow::anyhow!("unknown system capability: {}", capability.key())
    })
}

pub fn user_registry() -> BTreeMap<UserBridgeCapability, UserCapabilityDefinition> {
    UserBridgeCapability::ALL
        .iter()
        .copied()
        .map(|capability| {
            let definition = require_user_capability_definition(capability)
                .expect("all user capabilities must have definitions");
            (capability, definition)
        })
        .collect()
}

pub fn system_registry() -> BTreeMap<SystemBridgeCapability, SystemCapabilityDefinition> {
    SystemBridgeCapability::ALL
        .iter()
        .copied()
        .map(|capability| {
            let definition = require_system_capability_definition(capability)
                .expect("all system capabilities must have definitions");
            (capability, definition)
        })
        .collect()
}

pub fn get_user_capability_definition_by_key(
    key: &str,
) -> Option<UserCapabilityDefinition> {
    UserBridgeCapability::from_key(key).and_then(get_user_capability_definition)
}

pub fn get_system_capability_definition_by_key(
    key: &str,
) -> Option<SystemCapabilityDefinition> {
    SystemBridgeCapability::from_key(key).and_then(get_system_capability_definition)
}

pub fn require_user_capability_definition_by_key(
    key: &str,
) -> anyhow::Result<UserCapabilityDefinition> {
    get_user_capability_definition_by_key(key)
        .ok_or_else(|| anyhow::anyhow!("unknown user capability: {}", key))
}

pub fn require_system_capability_definition_by_key(
    key: &str,
) -> anyhow::Result<SystemCapabilityDefinition> {
    get_system_capability_definition_by_key(key)
        .ok_or_else(|| anyhow::anyhow!("unknown system capability: {}", key))
}

pub fn user_capability_definition_view(
    definition: UserCapabilityDefinition,
) -> SageAppCapabilityDefinitionView {
    SageAppCapabilityDefinitionView {
        key: definition.capability.key().to_string(),
        label: definition.label.to_string(),
        description: definition.description.to_string(),
        flags: SageAppCapabilityFlagsView {
            externally_observable: definition.flags.externally_observable,
            accesses_sensitive_secret: definition.flags.accesses_sensitive_secret,
            requestable_by_app: definition.flags.requestable_by_app,
            user_grantable: definition.flags.user_grantable,
        },
    }
}
