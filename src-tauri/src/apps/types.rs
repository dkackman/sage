use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SageAppPermissions {
    pub network: bool,
    #[serde(rename = "persistentStorage", alias = "persistent_storage")]
    pub persistent_storage: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq, PartialOrd, Ord)]
pub struct SageNetworkPermissionEntry {
    pub scheme: String,
    pub host: String,
    #[serde(default)]
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SagePersistentStoragePermission {
    #[serde(default)]
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, Default)]
pub struct SageRequestedPermissions {
    #[serde(default)]
    pub network: Vec<SageNetworkPermissionEntry>,

    #[serde(default)]
    pub persistent_storage: Option<SagePersistentStoragePermission>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, Default)]
pub struct SageGrantedPermissions {
    #[serde(default)]
    pub network: Vec<SageGrantedNetworkPermissionEntry>,

    #[serde(rename = "persistentStorage", alias = "persistent_storage", default)]
    pub persistent_storage: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq, PartialOrd, Ord)]
pub struct SageGrantedNetworkPermissionEntry {
    pub scheme: String,
    pub host: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SageAppPackageManifest {
    pub name: String,
    pub version: String,

    #[serde(default)]
    pub permissions: SageRequestedPermissions,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SageInstalledAppSource {
    Zip {
        #[serde(rename = "installDir", alias = "install_dir")]
        install_dir: String,
    },
    Url {
        #[serde(rename = "appUrl", alias = "app_url")]
        app_url: String,
        #[serde(rename = "manifestUrl", alias = "manifest_url")]
        manifest_url: String,
        #[serde(rename = "lastSeenManifestHash", alias = "last_seen_manifest_hash")]
        last_seen_manifest_hash: String,
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

    pub source: SageInstalledAppSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CorruptedInstalledSageApp {
    pub id: String,
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
    pub headers: BTreeMap<String, String>,
    #[serde(default)]
    pub body: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SageBridgeFetchResponse {
    pub ok: bool,
    pub status: u16,
    pub status_text: String,
    pub headers: BTreeMap<String, String>,
    pub body_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SageBridgeFetchBatchRequest {
    pub requests: Vec<SageBridgeFetchRequest>,
    #[serde(default)]
    pub max_concurrency: Option<usize>,
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
