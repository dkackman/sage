use serde::{Deserialize, Serialize};
use specta::Type;
use sage_api::Amount;

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq, PartialOrd, Ord)]
pub struct SageNetworkPermissionEntry {
    pub scheme: String,
    pub host: String,
    #[serde(default)]
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SagePermissionRequired {
    #[serde(default)]
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SageWalletPermissions {
    #[serde(rename = "sendXch", alias = "send_xch")]
    pub send_xch: Option<SagePermissionRequired>,

    #[serde(rename = "sendXchAutoSubmit", alias = "send_xch_auto_submit")]
    pub send_xch_auto_submit: Option<SagePermissionRequired>,
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

#[derive(Debug, Clone, Serialize, Deserialize, Type, Default)]
pub struct SageRequestedPermissions {
    #[serde(default)]
    pub network: Vec<SageNetworkPermissionEntry>,

    #[serde(default)]
    pub persistent_storage: Option<SagePermissionRequired>,

    pub wallet: Option<SageWalletPermissions>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, Default)]
pub struct SageGrantedWalletPermissions {
    #[serde(rename = "sendXch", alias = "send_xch")]
    pub send_xch: bool,

    #[serde(rename = "sendXchAutoSubmit", alias = "send_xch_auto_submit")]
    pub send_xch_auto_submit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, Default)]
pub struct SageGrantedPermissions {
    pub network: Vec<SageGrantedNetworkPermissionEntry>,

    #[serde(rename = "persistentStorage", alias = "persistent_storage")]
    pub persistent_storage: bool,

    pub wallet: SageGrantedWalletPermissions,
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
pub struct SageAppPackageManifest {
    pub name: String,
    pub version: String,

    #[serde(default)]
    pub permissions: SageRequestedPermissions,

    #[serde(default)]
    pub files: Vec<SageAppManifestFile>,
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
    pub requested_permissions: SageRequestedPermissions,

    #[serde(rename = "grantedPermissions", alias = "granted_permissions")]
    pub granted_permissions: SageGrantedPermissions,

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

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SageBridgeFetchRequest {
    pub url: String,
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub headers: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub body: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SageBridgeFetchResponse {
    pub ok: bool,
    pub status: u16,
    pub status_text: String,
    pub headers: std::collections::BTreeMap<String, String>,
    pub body_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SageBridgeFetchBatchRequest {
    pub requests: Vec<SageBridgeFetchRequest>,
    #[serde(default)]
    pub max_concurrency: Option<usize>,
}
