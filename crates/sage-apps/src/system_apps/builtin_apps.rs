use std::{fs, path::PathBuf};

use anyhow::{anyhow, Context, Result as AnyResult};
use sha2::{Digest, Sha256};
use tauri::command;

use crate::host::Result;
use crate::lifecycle::{manifest_entry_file, manifest_icon_file};
use crate::permissions::{
    normalize_and_validate_requested_permissions, resolve_capability_flags,
    validate_granted_capabilities,
};
use crate::types::{
    InstalledSageAppStorage, SageApp, SageAppCommon, SageAppSnapshot,
    SageAppPackageManifest, SageGrantedNetworkPermissions, SageGrantedPermissions,
    SystemAppPresentation, SystemSageApp,
};

pub const BUILTIN_SYSTEM_TASK_MANAGER_ID: &str = "task-manager";

#[derive(Debug, Clone, Copy)]
pub struct BuiltinSystemAppSpec {
    pub app_id: &'static str,
    pub dir_name: &'static str,
    pub presentation: SystemAppPresentation,
}

const BUILTIN_SYSTEM_APPS: &[BuiltinSystemAppSpec] = &[BuiltinSystemAppSpec {
    app_id: BUILTIN_SYSTEM_TASK_MANAGER_ID,
    dir_name: "task-manager",
    presentation: SystemAppPresentation::Taskbar,
}];

pub fn builtin_system_app_spec(app_id: &str) -> Option<&'static BuiltinSystemAppSpec> {
    BUILTIN_SYSTEM_APPS.iter().find(|spec| spec.app_id == app_id)
}

pub fn builtin_apps_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("builtin-apps")
        .join("dist")
}

pub fn builtin_system_apps_root() -> PathBuf {
    builtin_apps_root().join("system-apps")
}

pub fn builtin_system_app_dir(app_id: &str) -> AnyResult<Option<PathBuf>> {
    let Some(spec) = builtin_system_app_spec(app_id) else {
        return Ok(None);
    };

    Ok(Some(builtin_system_apps_root().join(spec.dir_name)))
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn builtin_storage(app_id: &str) -> InstalledSageAppStorage {
    let mut hasher = Sha256::new();
    hasher.update(format!("builtin-system-storage:{app_id}").as_bytes());
    let digest = hasher.finalize();

    InstalledSageAppStorage::AppleDataStore {
        identifier_hex: hex::encode(&digest[..16]),
    }
}

#[cfg(target_os = "windows")]
fn builtin_storage(app_id: &str) -> InstalledSageAppStorage {
    InstalledSageAppStorage::WindowsProfile {
        directory_name: format!("builtin-system-profile-{app_id}"),
    }
}

#[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "windows")))]
fn builtin_storage(_app_id: &str) -> InstalledSageAppStorage {
    InstalledSageAppStorage::Unmanaged
}

fn read_builtin_manifest(app_dir: &PathBuf) -> AnyResult<SageAppPackageManifest> {
    let manifest_path = app_dir.join("sage-manifest.json");

    let manifest_text = fs::read_to_string(&manifest_path).with_context(|| {
        format!(
            "failed to read builtin system app manifest {}",
            manifest_path.display()
        )
    })?;

    let manifest: SageAppPackageManifest =
        serde_json::from_str(&manifest_text).with_context(|| {
            format!(
                "failed to parse builtin system app manifest {}",
                manifest_path.display()
            )
        })?;

    Ok(manifest)
}

fn compute_total_bytes(app_dir: &PathBuf) -> AnyResult<u64> {
    let mut total_bytes = 0_u64;

    for entry in fs::read_dir(app_dir)
        .with_context(|| format!("failed to read builtin system app dir {}", app_dir.display()))?
    {
        let entry = entry.with_context(|| {
            format!(
                "failed to read entry in builtin system app dir {}",
                app_dir.display()
            )
        })?;

        let metadata = entry.metadata().with_context(|| {
            format!(
                "failed to read metadata for builtin system app file {}",
                entry.path().display()
            )
        })?;

        if metadata.is_file() {
            total_bytes = total_bytes
                .checked_add(metadata.len())
                .ok_or_else(|| anyhow!("builtin system app total size overflow"))?;
        }
    }

    Ok(total_bytes)
}

pub fn build_builtin_system_app(app_id: &str) -> AnyResult<Option<SageApp>> {
    let Some(spec) = builtin_system_app_spec(app_id) else {
        return Ok(None);
    };

    let app_dir = builtin_system_apps_root().join(spec.dir_name);

    if !app_dir.is_dir() {
        return Err(anyhow!(
            "builtin system app directory does not exist: {}",
            app_dir.display()
        ));
    }

    let mut manifest = read_builtin_manifest(&app_dir)?;
    manifest.permissions =
        normalize_and_validate_requested_permissions(&manifest.permissions)?;

    let mut requested_capabilities = manifest.permissions.capabilities.required.clone();
    requested_capabilities.extend(manifest.permissions.capabilities.optional.clone());
    requested_capabilities.sort();
    requested_capabilities.dedup();

    let granted_permissions = SageGrantedPermissions {
        capabilities: requested_capabilities,
        network: SageGrantedNetworkPermissions {
            whitelist: manifest.permissions.network.whitelist.required.clone(),
        },
    };

    validate_granted_capabilities(
        &manifest.permissions,
        &granted_permissions.capabilities,
    )?;
    let permission_flags =
        resolve_capability_flags(&granted_permissions.capabilities, None)?;

    let entry_file_name = manifest_entry_file(&manifest).to_string();
    let icon_file_name = manifest_icon_file(&manifest).to_string();

    let entry_file = app_dir.join(&entry_file_name);
    if !entry_file.is_file() {
        return Err(anyhow!(
            "builtin system app entry file does not exist: {}",
            entry_file.display()
        ));
    }

    let icon_file = app_dir.join(&icon_file_name);
    if !icon_file.is_file() {
        return Err(anyhow!(
            "builtin system app icon file does not exist: {}",
            icon_file.display()
        ));
    }

    let total_bytes = compute_total_bytes(&app_dir)?;

    let app = SystemSageApp {
        common: SageAppCommon {
            id: spec.app_id.to_string(),
            origin_id: spec.app_id.to_string(),
            name: manifest.name.clone(),
            version: manifest.version.clone(),
            app_dir: app_dir.to_string_lossy().to_string(),
            entry_file: entry_file_name,
            icon_file: icon_file_name,
            requested_permissions: manifest.permissions.clone(),
            granted_permissions,
            capability_flags: permission_flags,
            storage: builtin_storage(spec.app_id),
            active_snapshot: SageAppSnapshot {
                manifest_hash: format!("builtin-system:{}", spec.app_id),
                snapshot_dir: app_dir.to_string_lossy().to_string(),
                total_bytes,
                manifest,
            },
        },
        presentation: spec.presentation,
    };

    Ok(Some(SageApp::System(app)))
}

#[command]
#[specta::specta]
pub async fn get_builtin_system_app(
    app_id: String,
) -> Result<Option<SageApp>> {
    build_builtin_system_app(&app_id).map_err(|err| {
        std::io::Error::other(format!("failed to load builtin system app: {err}")).into()
    })
}
