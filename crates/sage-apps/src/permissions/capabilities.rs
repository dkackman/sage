use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy)]
pub struct CapabilityFlags {
    pub externally_observable: bool,
    pub accesses_sensitive_secret: bool,
    pub persistent_storage: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct CapabilityDefinition {
    pub key: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub flags: CapabilityFlags,
    pub requestable_by_app: bool,
    pub shared_with_app: bool,
}

pub fn get_capability_definition(key: &str) -> Option<CapabilityDefinition> {
    registry().get(key).copied()
}

pub fn require_capability_definition(
    key: &str,
) -> anyhow::Result<CapabilityDefinition> {
    get_capability_definition(key)
        .ok_or_else(|| anyhow::anyhow!("unknown capability: {}", key))
}

pub fn registry() -> BTreeMap<&'static str, CapabilityDefinition> {
    let mut map = BTreeMap::new();

    map.insert(
        "persistent_storage",
        CapabilityDefinition {
            key: "persistent_storage",
            label: "Persistent storage",
            description:
            "Allows the app to store data on this device between sessions.",
            flags: CapabilityFlags {
                externally_observable: false,
                accesses_sensitive_secret: false,
                persistent_storage: true,
            },
            requestable_by_app: true,
            shared_with_app: false,
        },
    );

    map.insert(
        "wallet.send_xch",
        CapabilityDefinition {
            key: "wallet.send_xch",
            label: "Send XCH",
            description:
            "Allows the app to request XCH transactions from your wallet.",
            flags: CapabilityFlags {
                externally_observable: true,
                accesses_sensitive_secret: false,
                persistent_storage: false,
            },
            requestable_by_app: true,
            shared_with_app: true,
        },
    );

    map.insert(
        "wallet.send_xch_auto_submit",
        CapabilityDefinition {
            key: "wallet.send_xch_auto_submit",
            label: "Automatic XCH send",
            description:
            "Allows the app to submit XCH transactions without asking for per-transaction approval.",
            flags: CapabilityFlags {
                externally_observable: false,
                accesses_sensitive_secret: false,
                persistent_storage: false,
            },
            requestable_by_app: false,
            shared_with_app: false,
        },
    );

    map
}
