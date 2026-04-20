use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result as AnyResult};
use serde::{Deserialize, Serialize};

use crate::apps::types::{
    CorruptedInstalledSageApp, InstalledSageApp, InstalledSageAppPendingUpdate,
    InstalledSageAppCapabilityFlags, InstalledSageAppSnapshot, InstalledSageAppSource,
    ListedSageApp, SageAppManifestFile, SageAppPackageManifest, SageGrantedPermissions,
    SageNetworkPermissionTarget, SageRequestedCapabilities, SageRequestedNetworkPermissions,
    SageRequestedNetworkWhitelist, SageRequestedPermissions,
};

const INSTALLED_METADATA_FILE: &str = ".sage-installed.json";

pub fn apps_root(base_path: &Path) -> PathBuf {
    base_path.join("apps")
}

pub fn app_install_dir(base_path: &Path, app_id: &str) -> PathBuf {
    apps_root(base_path).join(app_id)
}

pub fn installed_metadata_path(install_dir: &Path) -> PathBuf {
    install_dir.join(INSTALLED_METADATA_FILE)
}

#[derive(Debug, Serialize, Deserialize)]
struct PersistedStringListBucket {
    required: Vec<String>,
    optional: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PersistedRequestedNetworkPermissions {
    whitelist: PersistedStringListBucket,
}

#[derive(Debug, Serialize, Deserialize)]
struct PersistedRequestedPermissions {
    network: PersistedRequestedNetworkPermissions,
    capabilities: SageRequestedCapabilities,
}

#[derive(Debug, Serialize, Deserialize)]
struct PersistedSageAppPackageManifest {
    name: String,
    version: String,
    permissions: PersistedRequestedPermissions,
    files: Vec<SageAppManifestFile>,
    entry: Option<String>,
    icon: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PersistedInstalledSageAppSnapshot {
    #[serde(rename = "manifestHash", alias = "manifest_hash")]
    manifest_hash: String,

    #[serde(rename = "snapshotDir", alias = "snapshot_dir")]
    snapshot_dir: String,

    #[serde(rename = "totalBytes", alias = "total_bytes")]
    total_bytes: u64,

    manifest: PersistedSageAppPackageManifest,
}

#[derive(Debug, Serialize, Deserialize)]
struct PersistedInstalledSageAppPendingUpdate {
    #[serde(rename = "appUrl", alias = "app_url")]
    app_url: String,

    #[serde(rename = "manifestUrl", alias = "manifest_url")]
    manifest_url: String,

    #[serde(rename = "manifestHash", alias = "manifest_hash")]
    manifest_hash: String,

    manifest: PersistedSageAppPackageManifest,
}

#[derive(Debug, Serialize, Deserialize)]
struct PersistedInstalledSageApp {
    id: String,
    name: String,
    version: String,

    #[serde(rename = "installDir", alias = "install_dir")]
    install_dir: String,

    #[serde(rename = "entryFile", alias = "entry_file")]
    entry_file: String,

    #[serde(rename = "iconFile", alias = "icon_file")]
    icon_file: String,

    #[serde(rename = "requestedPermissions", alias = "requested_permissions")]
    requested_permissions: PersistedRequestedPermissions,

    #[serde(rename = "grantedPermissions", alias = "granted_permissions")]
    granted_permissions: SageGrantedPermissions,

    #[serde(rename = "permissionFlags", alias = "permission_flags")]
    permission_flags: InstalledSageAppCapabilityFlags,

    source: InstalledSageAppSource,

    #[serde(rename = "activeSnapshot", alias = "active_snapshot")]
    active_snapshot: PersistedInstalledSageAppSnapshot,

    #[serde(rename = "pendingUpdate", alias = "pending_update")]
    pending_update: Option<PersistedInstalledSageAppPendingUpdate>,
}

fn format_network_target(value: &SageNetworkPermissionTarget) -> String {
    format!("{}://{}", value.scheme, value.host)
}

pub fn parse_network_permission_target(
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

fn to_persisted_requested_permissions(
    value: &SageRequestedPermissions,
) -> PersistedRequestedPermissions {
    PersistedRequestedPermissions {
        network: PersistedRequestedNetworkPermissions {
            whitelist: PersistedStringListBucket {
                required: value
                    .network
                    .whitelist
                    .required
                    .iter()
                    .map(format_network_target)
                    .collect(),
                optional: value
                    .network
                    .whitelist
                    .optional
                    .iter()
                    .map(format_network_target)
                    .collect(),
            },
        },
        capabilities: value.capabilities.clone(),
    }
}

fn from_persisted_requested_permissions(
    value: PersistedRequestedPermissions,
) -> AnyResult<SageRequestedPermissions> {
    let required = value
        .network
        .whitelist
        .required
        .into_iter()
        .map(|entry| parse_network_permission_target(&entry))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| anyhow::anyhow!("failed to parse persisted required network entry: {err}"))?;

    let optional = value
        .network
        .whitelist
        .optional
        .into_iter()
        .map(|entry| parse_network_permission_target(&entry))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| anyhow::anyhow!("failed to parse persisted optional network entry: {err}"))?;

    Ok(SageRequestedPermissions {
        network: SageRequestedNetworkPermissions {
            whitelist: SageRequestedNetworkWhitelist { required, optional },
        },
        capabilities: value.capabilities,
    })
}

fn to_persisted_manifest(
    value: &SageAppPackageManifest,
) -> PersistedSageAppPackageManifest {
    PersistedSageAppPackageManifest {
        name: value.name.clone(),
        version: value.version.clone(),
        permissions: to_persisted_requested_permissions(&value.permissions),
        files: value.files.clone(),
        entry: value.entry.clone(),
        icon: value.icon.clone(),
    }
}

fn from_persisted_manifest(
    value: PersistedSageAppPackageManifest,
) -> AnyResult<SageAppPackageManifest> {
    Ok(SageAppPackageManifest {
        name: value.name,
        version: value.version,
        permissions: from_persisted_requested_permissions(value.permissions)?,
        files: value.files,
        entry: value.entry,
        icon: value.icon,
    })
}

fn to_persisted_snapshot(
    value: &InstalledSageAppSnapshot,
) -> PersistedInstalledSageAppSnapshot {
    PersistedInstalledSageAppSnapshot {
        manifest_hash: value.manifest_hash.clone(),
        snapshot_dir: value.snapshot_dir.clone(),
        total_bytes: value.total_bytes,
        manifest: to_persisted_manifest(&value.manifest),
    }
}

fn from_persisted_snapshot(
    value: PersistedInstalledSageAppSnapshot,
) -> AnyResult<InstalledSageAppSnapshot> {
    Ok(InstalledSageAppSnapshot {
        manifest_hash: value.manifest_hash,
        snapshot_dir: value.snapshot_dir,
        total_bytes: value.total_bytes,
        manifest: from_persisted_manifest(value.manifest)?,
    })
}

fn to_persisted_pending_update(
    value: &InstalledSageAppPendingUpdate,
) -> PersistedInstalledSageAppPendingUpdate {
    PersistedInstalledSageAppPendingUpdate {
        app_url: value.app_url.clone(),
        manifest_url: value.manifest_url.clone(),
        manifest_hash: value.manifest_hash.clone(),
        manifest: to_persisted_manifest(&value.manifest),
    }
}

fn from_persisted_pending_update(
    value: PersistedInstalledSageAppPendingUpdate,
) -> AnyResult<InstalledSageAppPendingUpdate> {
    Ok(InstalledSageAppPendingUpdate {
        app_url: value.app_url,
        manifest_url: value.manifest_url,
        manifest_hash: value.manifest_hash,
        manifest: from_persisted_manifest(value.manifest)?,
    })
}

fn to_persisted_installed_app(app: &InstalledSageApp) -> PersistedInstalledSageApp {
    PersistedInstalledSageApp {
        id: app.id.clone(),
        name: app.name.clone(),
        version: app.version.clone(),
        install_dir: app.install_dir.clone(),
        entry_file: app.entry_file.clone(),
        icon_file: app.icon_file.clone(),
        requested_permissions: to_persisted_requested_permissions(
            &app.requested_permissions,
        ),
        granted_permissions: app.granted_permissions.clone(),
        permission_flags: app.permission_flags.clone(),
        source: app.source.clone(),
        active_snapshot: to_persisted_snapshot(&app.active_snapshot),
        pending_update: app
            .pending_update
            .as_ref()
            .map(to_persisted_pending_update),
    }
}

fn from_persisted_installed_app(
    app: PersistedInstalledSageApp,
) -> AnyResult<InstalledSageApp> {
    Ok(InstalledSageApp {
        id: app.id,
        name: app.name,
        version: app.version,
        install_dir: app.install_dir,
        entry_file: app.entry_file,
        icon_file: app.icon_file,
        requested_permissions: from_persisted_requested_permissions(
            app.requested_permissions,
        )?,
        granted_permissions: app.granted_permissions,
        permission_flags: app.permission_flags,
        source: app.source,
        active_snapshot: from_persisted_snapshot(app.active_snapshot)?,
        pending_update: app
            .pending_update
            .map(from_persisted_pending_update)
            .transpose()?,
    })
}

pub fn read_installed_app_from_dir(dir: &Path) -> AnyResult<InstalledSageApp> {
    let path = installed_metadata_path(dir);
    let text =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let persisted: PersistedInstalledSageApp =
        serde_json::from_str(&text).context("failed to parse installed app metadata")?;
    from_persisted_installed_app(persisted)
}

pub fn write_installed_app_metadata(
    app: &InstalledSageApp,
    install_dir: &Path,
) -> AnyResult<()> {
    let path = installed_metadata_path(install_dir);
    let persisted = to_persisted_installed_app(app);
    let text = serde_json::to_string_pretty(&persisted)
        .map_err(|err| anyhow::anyhow!("failed to serialize installed app metadata: {err}"))?;
    fs::write(path, format!("{text}\n"))?;
    Ok(())
}

pub fn read_installed_app_by_id(
    base_path: &Path,
    app_id: &str,
) -> AnyResult<InstalledSageApp> {
    let install_dir = app_install_dir(base_path, app_id);
    read_installed_app_from_dir(&install_dir)
}

pub fn list_installed_apps_internal(root: &Path) -> AnyResult<Vec<ListedSageApp>> {
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut apps = Vec::new();

    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();

        if !entry.file_type()?.is_dir() {
            continue;
        }

        if path
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.starts_with(".tmp-"))
            .unwrap_or(false)
        {
            continue;
        }

        let Some(id) = path
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
        else {
            continue;
        };

        let metadata_path = installed_metadata_path(&path);
        if !metadata_path.is_file() {
            continue;
        }

        match read_installed_app_from_dir(&path) {
            Ok(app) => apps.push(ListedSageApp::Installed(app)),
            Err(err) => apps.push(ListedSageApp::Corrupted(CorruptedInstalledSageApp {
                id,
                install_dir: path.to_string_lossy().to_string(),
                error: err.to_string(),
            })),
        }
    }

    apps.sort_by(|a, b| {
        let a_key = match a {
            ListedSageApp::Installed(app) => app.name.to_lowercase(),
            ListedSageApp::Corrupted(app) => app.id.to_lowercase(),
        };

        let b_key = match b {
            ListedSageApp::Installed(app) => app.name.to_lowercase(),
            ListedSageApp::Corrupted(app) => app.id.to_lowercase(),
        };

        a_key.cmp(&b_key)
    });

    Ok(apps)
}
