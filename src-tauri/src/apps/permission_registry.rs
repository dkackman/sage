use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy)]
pub struct PermissionFlags {
    pub externally_observable: bool,
    pub accesses_sensitive_secret: bool,
    pub persistent_storage: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct PermissionDefinition {
    pub key: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub flags: PermissionFlags,
}

pub fn get_permission_definition(key: &str) -> Option<PermissionDefinition> {
    registry().get(key).copied()
}

pub fn require_permission_definition(key: &str) -> anyhow::Result<PermissionDefinition> {
    get_permission_definition(key)
        .ok_or_else(|| anyhow::anyhow!("unknown permission: {}", key))
}

pub fn registry() -> BTreeMap<&'static str, PermissionDefinition> {
    let mut map = BTreeMap::new();

    map.insert(
        "persistent_storage",
        PermissionDefinition {
            key: "persistent_storage",
            label: "Persistent storage",
            description: "Allows the app to store data on this device between sessions.",
            flags: PermissionFlags {
                externally_observable: false,
                accesses_sensitive_secret: false,
                persistent_storage: true,
            },
        },
    );

    map.insert(
        "wallet.send_xch",
        PermissionDefinition {
            key: "wallet.send_xch",
            label: "Send XCH",
            description: "Allows the app to request XCH transactions from your wallet.",
            flags: PermissionFlags {
                externally_observable: true,
                accesses_sensitive_secret: false,
                persistent_storage: false,
            },
        },
    );

    map.insert(
        "wallet.send_xch_auto_submit",
        PermissionDefinition {
            key: "wallet.send_xch_auto_submit",
            label: "Automatic XCH send",
            description: "Allows the app to submit XCH transactions without asking for per-transaction approval.",
            flags: PermissionFlags {
                externally_observable: false,
                accesses_sensitive_secret: false,
                persistent_storage: false,
            },
        },
    );

    map
}
