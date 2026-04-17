use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type, Default)]
pub struct SageAppPermissions {
    pub required: Vec<String>,
    pub optional: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq, PartialOrd, Ord)]
pub struct SageNetworkWhitelistEntry {
    pub scheme: String,
    pub host: String,

    #[serde(default)]
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, Default)]
pub struct SageNetworkPermissions {
    #[serde(default)]
    pub whitelist: Vec<SageNetworkWhitelistEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SageAppPackageManifest {
    pub name: String,
    pub version: String,

    #[serde(default)]
    pub permissions: SageAppPermissions,

    #[serde(default)]
    pub network: Option<SageNetworkPermissions>,

    #[serde(default)]
    pub files: Vec<SageAppManifestFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct SageAppUrlPreview {
    #[serde(rename = "appUrl", alias = "app_url")]
    pub app_url: String,

    #[serde(rename = "manifestUrl", alias = "manifest_url")]
    pub manifest_url: String,

    #[serde(rename = "manifestHash", alias = "manifest_hash")]
    pub manifest_hash: String,

    pub manifest: SageAppPackageManifest,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq, PartialOrd, Ord)]
pub struct SageGrantedNetworkPermissionEntry {
    pub scheme: String,
    pub host: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
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

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct InstalledSageAppPendingUpdate {
    #[serde(rename = "appUrl", alias = "app_url")]
    pub app_url: String,

    #[serde(rename = "manifestUrl", alias = "manifest_url")]
    pub manifest_url: String,

    #[serde(rename = "manifestHash", alias = "manifest_hash")]
    pub manifest_hash: String,

    pub manifest: SageAppPackageManifest,

    pub snapshot: InstalledSageAppSnapshot,
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
    pub requested_permissions: SageAppPermissions,

    #[serde(rename = "grantedPermissions", alias = "granted_permissions")]
    pub granted_permissions: Vec<String>,

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
