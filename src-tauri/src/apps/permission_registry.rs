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
    pub flags: PermissionFlags,
}

pub fn registry() -> BTreeMap<&'static str, PermissionDefinition> {
    let mut map = BTreeMap::new();

    map.insert(
        "persistent_storage",
        PermissionDefinition {
            key: "persistent_storage",
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
            flags: PermissionFlags {
                externally_observable: true,
                accesses_sensitive_secret: true,
                persistent_storage: false,
            },
        },
    );

    map.insert(
        "wallet.send_xch_auto_submit",
        PermissionDefinition {
            key: "wallet.send_xch_auto_submit",
            flags: PermissionFlags {
                externally_observable: true,
                accesses_sensitive_secret: true,
                persistent_storage: false,
            },
        },
    );

    map
}
