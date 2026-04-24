use std::collections::BTreeMap;

use crate::bridge::capabilities::{SystemBridgeCapability, UserBridgeCapability};
use crate::types::{SageAppCapabilityDefinitionView, SageAppCapabilityFlagsView};

#[derive(Debug, Clone, Copy)]
pub struct CapabilityFlags {
    pub externally_observable: bool,
    pub accesses_sensitive_secret: bool,
    pub persistent_storage: bool,
    pub requestable_by_app: bool,
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

const USER_CAPABILITY_DEFINITIONS: &[UserCapabilityDefinition] = &[
    UserCapabilityDefinition {
        capability: UserBridgeCapability::PersistentStorage,
        label: "Persistent storage",
        description: "Allows the app to store data on this device between sessions.",
        flags: CapabilityFlags {
            externally_observable: false,
            accesses_sensitive_secret: false,
            persistent_storage: true,
            requestable_by_app: true,
            shared_with_app: false,
        },
    },
    UserCapabilityDefinition {
        capability: UserBridgeCapability::WalletSendXch,
        label: "Send XCH",
        description: "Allows the app to request XCH transactions from your wallet.",
        flags: CapabilityFlags {
            externally_observable: true,
            accesses_sensitive_secret: false,
            persistent_storage: false,
            requestable_by_app: true,
            shared_with_app: true,
        },
    },
    UserCapabilityDefinition {
        capability: UserBridgeCapability::WalletSendXchAutoSubmit,
        label: "Automatic XCH send",
        description: "Allows the app to submit XCH transactions without asking for per-transaction approval.",
        flags: CapabilityFlags {
            externally_observable: false,
            accesses_sensitive_secret: false,
            persistent_storage: false,
            requestable_by_app: false,
            shared_with_app: false,
        },
    },
];

const SYSTEM_CAPABILITY_DEFINITIONS: &[SystemCapabilityDefinition] = &[
    SystemCapabilityDefinition {
        capability: SystemBridgeCapability::RuntimeManagerListRuntimes,
        label: "List app runtimes",
        description: "Allows the system app to inspect running Sage app runtimes.",
        flags: CapabilityFlags {
            externally_observable: false,
            accesses_sensitive_secret: false,
            persistent_storage: false,
            requestable_by_app: false,
            shared_with_app: true,
        },
    },
    SystemCapabilityDefinition {
        capability: SystemBridgeCapability::RuntimeManagerFocusRuntime,
        label: "Focus app runtimes",
        description: "Allows the system app to focus running Sage app runtimes.",
        flags: CapabilityFlags {
            externally_observable: true,
            accesses_sensitive_secret: false,
            persistent_storage: false,
            requestable_by_app: false,
            shared_with_app: true,
        },
    },
    SystemCapabilityDefinition {
        capability: SystemBridgeCapability::RuntimeManagerHideRuntime,
        label: "Hide app runtimes",
        description: "Allows the system app to hide running Sage app runtimes.",
        flags: CapabilityFlags {
            externally_observable: true,
            accesses_sensitive_secret: false,
            persistent_storage: false,
            requestable_by_app: false,
            shared_with_app: true,
        },
    },
    SystemCapabilityDefinition {
        capability: SystemBridgeCapability::RuntimeManagerKillRuntime,
        label: "Kill app runtimes",
        description: "Allows the system app to stop running Sage app runtimes.",
        flags: CapabilityFlags {
            externally_observable: true,
            accesses_sensitive_secret: false,
            persistent_storage: false,
            requestable_by_app: false,
            shared_with_app: true,
        },
    },
    SystemCapabilityDefinition {
        capability: SystemBridgeCapability::RuntimeManagerListenRuntimesChanged,
        label: "Observe runtime changes",
        description: "Allows the system app to receive events when Sage app runtimes change.",
        flags: CapabilityFlags {
            externally_observable: false,
            accesses_sensitive_secret: false,
            persistent_storage: false,
            requestable_by_app: false,
            shared_with_app: true,
        },
    },
];

fn registry_from_definitions<C>(
    definitions: &'static [CapabilityDefinition<C>],
) -> BTreeMap<C, CapabilityDefinition<C>>
where
    C: Copy + Ord,
{
    definitions
        .iter()
        .copied()
        .map(|definition| (definition.capability, definition))
        .collect()
}

pub fn user_registry() -> BTreeMap<UserBridgeCapability, UserCapabilityDefinition> {
    registry_from_definitions(USER_CAPABILITY_DEFINITIONS)
}

pub fn system_registry() -> BTreeMap<SystemBridgeCapability, SystemCapabilityDefinition> {
    registry_from_definitions(SYSTEM_CAPABILITY_DEFINITIONS)
}

pub fn get_user_capability_definition(
    capability: UserBridgeCapability,
) -> Option<UserCapabilityDefinition> {
    user_registry().get(&capability).copied()
}

pub fn require_user_capability_definition(
    capability: UserBridgeCapability,
) -> anyhow::Result<UserCapabilityDefinition> {
    get_user_capability_definition(capability).ok_or_else(|| {
        anyhow::anyhow!("unknown user capability: {}", capability.key())
    })
}

pub fn get_system_capability_definition(
    capability: SystemBridgeCapability,
) -> Option<SystemCapabilityDefinition> {
    system_registry().get(&capability).copied()
}

pub fn require_system_capability_definition(
    capability: SystemBridgeCapability,
) -> anyhow::Result<SystemCapabilityDefinition> {
    get_system_capability_definition(capability).ok_or_else(|| {
        anyhow::anyhow!("unknown system capability: {}", capability.key())
    })
}

pub fn get_user_capability_definition_by_key(
    key: &str,
) -> Option<UserCapabilityDefinition> {
    USER_CAPABILITY_DEFINITIONS
        .iter()
        .copied()
        .find(|definition| definition.capability.key() == key)
}

pub fn get_system_capability_definition_by_key(
    key: &str,
) -> Option<SystemCapabilityDefinition> {
    SYSTEM_CAPABILITY_DEFINITIONS
        .iter()
        .copied()
        .find(|definition| definition.capability.key() == key)
}

pub fn require_user_capability_definition_by_key(
    key: &str,
) -> anyhow::Result<UserCapabilityDefinition> {
    get_user_capability_definition_by_key(key)
        .ok_or_else(|| anyhow::anyhow!("unknown user capability: {}", key))
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
            persistent_storage: definition.flags.persistent_storage,
            requestable_by_app: definition.flags.requestable_by_app,
            shared_with_app: definition.flags.shared_with_app,
        },
    }
}
