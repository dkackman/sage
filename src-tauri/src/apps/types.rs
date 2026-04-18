use serde::{Deserialize, Deserializer, Serialize};
use specta::Type;

#[derive(
    Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq, PartialOrd, Ord,
)]
pub struct SageNetworkPermissionTarget {
    pub scheme: String,
    pub host: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, Default, PartialEq, Eq)]
pub struct SageRequestedNetworkWhitelist {
    #[serde(default)]
    pub required: Vec<SageNetworkPermissionTarget>,

    #[serde(default)]
    pub optional: Vec<SageNetworkPermissionTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, Default, PartialEq, Eq)]
pub struct SageRequestedNetworkPermissions {
    #[serde(default)]
    pub whitelist: SageRequestedNetworkWhitelist,
}

#[derive(
    Debug, Clone, Serialize, Deserialize, Type, Default, PartialEq, Eq,
)]
pub struct SageRequestedCapabilities {
    #[serde(default)]
    pub required: Vec<String>,

    #[serde(default)]
    pub optional: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Type, Default, PartialEq, Eq)]
pub struct SageRequestedPermissions {
    #[serde(default)]
    pub network: SageRequestedNetworkPermissions,

    #[serde(default)]
    pub capabilities: SageRequestedCapabilities,
}

#[derive(
    Debug, Clone, Serialize, Deserialize, Type, Default, PartialEq, Eq,
)]
pub struct SageGrantedNetworkPermissions {
    pub whitelist: Vec<SageNetworkPermissionTarget>,
}

#[derive(
    Debug, Clone, Serialize, Deserialize, Type, Default, PartialEq, Eq,
)]
pub struct SageGrantedPermissions {
    pub capabilities: Vec<String>,
    pub network: SageGrantedNetworkPermissions,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct SageAppPackageManifest {
    pub name: String,
    pub version: String,

    #[serde(default)]
    pub permissions: SageRequestedPermissions,

    #[serde(default)]
    pub files: Vec<SageAppManifestFile>,

    #[serde(default)]
    pub entry: Option<String>,

    #[serde(default)]
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SageAppUrlPreview {
    #[serde(rename = "appUrl", alias = "app_url")]
    pub app_url: String,

    #[serde(rename = "manifestUrl", alias = "manifest_url")]
    pub manifest_url: String,

    #[serde(rename = "manifestHash", alias = "manifest_hash")]
    pub manifest_hash: String,

    pub manifest: SageAppPackageManifest,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct SageAppManifestFile {
    pub path: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct InstalledSageAppSnapshot {
    #[serde(rename = "manifestHash", alias = "manifest_hash")]
    pub manifest_hash: String,

    #[serde(rename = "snapshotDir", alias = "snapshot_dir")]
    pub snapshot_dir: String,

    #[serde(rename = "totalBytes", alias = "total_bytes")]
    pub total_bytes: u64,

    pub manifest: SageAppPackageManifest,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, Default)]
pub struct InstalledSageAppPermissionFlags {
    #[serde(rename = "hasSecretAccess", alias = "has_secret_access")]
    pub has_secret_access: bool,

    #[serde(rename = "hasExternalAccess", alias = "has_external_access")]
    pub has_external_access: bool,

    #[serde(
        rename = "storageMayContainSecrets",
        alias = "storage_may_contain_secrets"
    )]
    pub storage_may_contain_secrets: bool,

    pub isolated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct InstalledSageAppPendingUpdate {
    #[serde(rename = "appUrl", alias = "app_url")]
    pub app_url: String,

    #[serde(rename = "manifestUrl", alias = "manifest_url")]
    pub manifest_url: String,

    #[serde(rename = "manifestHash", alias = "manifest_hash")]
    pub manifest_hash: String,

    pub manifest: SageAppPackageManifest,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum InstalledSageAppSource {
    Zip,
    Url {
        #[serde(rename = "appUrl", alias = "app_url")]
        app_url: String,
        #[serde(rename = "manifestUrl", alias = "manifest_url")]
        manifest_url: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct InstalledSageApp {
    pub id: String,
    pub name: String,
    pub version: String,

    #[serde(rename = "installDir", alias = "install_dir")]
    pub install_dir: String,

    #[serde(rename = "entryFile", alias = "entry_file")]
    pub entry_file: String,

    #[serde(rename = "iconFile", alias = "icon_file")]
    pub icon_file: String,

    #[serde(rename = "requestedPermissions", alias = "requested_permissions")]
    pub requested_permissions: SageRequestedPermissions,

    #[serde(rename = "grantedPermissions", alias = "granted_permissions")]
    pub granted_permissions: SageGrantedPermissions,

    #[serde(rename = "permissionFlags", alias = "permission_flags")]
    pub permission_flags: InstalledSageAppPermissionFlags,

    pub source: InstalledSageAppSource,

    #[serde(rename = "activeSnapshot", alias = "active_snapshot")]
    pub active_snapshot: InstalledSageAppSnapshot,

    #[serde(rename = "pendingUpdate", alias = "pending_update")]
    pub pending_update: Option<InstalledSageAppPendingUpdate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CorruptedInstalledSageApp {
    pub id: String,

    #[serde(rename = "installDir", alias = "install_dir")]
    pub install_dir: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ListedSageApp {
    Installed(InstalledSageApp),
    Corrupted(CorruptedInstalledSageApp),
}

#[derive(Debug, Deserialize, Default)]
struct RawStringListBucket {
    #[serde(default)]
    required: Vec<String>,

    #[serde(default)]
    optional: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
struct RawRequestedNetworkPermissions {
    #[serde(default)]
    whitelist: RawStringListBucket,
}

#[derive(Debug, Deserialize, Default)]
struct RawRequestedPermissions {
    #[serde(default)]
    network: RawRequestedNetworkPermissions,

    #[serde(default)]
    capabilities: Option<SageRequestedCapabilities>,
}

impl<'de> Deserialize<'de> for SageRequestedPermissions {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = <RawRequestedPermissions as Deserialize>::deserialize(deserializer)?;

        let required_network = raw
            .network
            .whitelist
            .required
            .into_iter()
            .map(|value| {
                parse_network_permission_target(&value)
                    .map_err(serde::de::Error::custom)
            })
            .collect::<Result<Vec<_>, _>>()?;

        let optional_network = raw
            .network
            .whitelist
            .optional
            .into_iter()
            .map(|value| {
                parse_network_permission_target(&value)
                    .map_err(serde::de::Error::custom)
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(SageRequestedPermissions {
            network: SageRequestedNetworkPermissions {
                whitelist: SageRequestedNetworkWhitelist {
                    required: required_network,
                    optional: optional_network,
                },
            },
            capabilities: raw.capabilities.unwrap_or_default(),
        })
    }
}

fn parse_network_permission_target(
    value: &str,
) -> Result<SageNetworkPermissionTarget, String> {
    let value = value.trim().to_ascii_lowercase();

    let (scheme, host) = value
        .split_once("://")
        .ok_or_else(|| format!("invalid network entry (missing scheme): {}", value))?;

    if scheme != "https" && scheme != "wss" {
        return Err(format!(
            "invalid scheme '{}', only https and wss allowed",
            scheme
        ));
    }

    if host.is_empty()
        || host.contains('/')
        || host.contains('?')
        || host.contains('#')
        || host.contains(' ')
    {
        return Err(format!("invalid host in network entry: {}", value));
    }

    Ok(SageNetworkPermissionTarget {
        scheme: scheme.to_string(),
        host: host.to_string(),
    })
}
