use std::{fs, path::{Path, PathBuf}};

use anyhow::Result as AnyResult;
use async_trait::async_trait;
use tauri::AppHandle;

use crate::lifecycle::{
    allocate_new_storage, apps_root, manifest_entry_file, manifest_icon_file,
    write_installed_app_metadata,
};
use crate::permissions::{
    normalize_and_validate_granted_permissions,
    resolve_capability_flags,
    resolve_effective_granted_capabilities,
};
use crate::types::{
    InstalledSageAppStorage, SageAppCapabilityFlags, SageAppCommon,
    SageAppPackageManifest, SageAppSnapshot, SageGrantedPermissions,
    UserSageApp, UserSageAppSource,
};

pub mod url;
pub mod zip;
pub mod commands;

#[async_trait]
pub trait AppInstallSource {
    type Prepared: Send + Sync;

    async fn prepare(&self) -> AnyResult<Self::Prepared>;

    fn manifest<'a>(&self, prepared: &'a Self::Prepared) -> &'a SageAppPackageManifest;

    fn source(&self, prepared: &Self::Prepared) -> UserSageAppSource;

    fn resolve_target(
        &self,
        root: &Path,
        base_path: &Path,
        prepared: &Self::Prepared,
    ) -> AnyResult<(String, PathBuf, Option<UserSageApp>)>;

    async fn create_snapshot(
        &self,
        app_dir: &Path,
        prepared: &Self::Prepared,
    ) -> AnyResult<SageAppSnapshot>;

    fn origin_id(
        &self,
        _base_path: &Path,
        app_id: &str,
        existing: Option<&UserSageApp>,
    ) -> AnyResult<String> {
        Ok(existing
            .map(|app| app.common.origin_id.clone())
            .unwrap_or_else(|| app_id.to_string()))
    }
}

pub async fn install_app_from_source<S>(
    app: &AppHandle,
    base_path: &Path,
    granted_permissions: SageGrantedPermissions,
    source: S,
) -> AnyResult<UserSageApp>
where
    S: AppInstallSource + Send + Sync,
{
    let root = apps_root(base_path);
    fs::create_dir_all(&root)?;

    let prepared = source.prepare().await?;
    let manifest = source.manifest(&prepared);

    let granted_permissions =
        normalize_and_validate_granted_permissions(&manifest.permissions, granted_permissions)?;

    let effective_capabilities = resolve_effective_granted_capabilities(
        &manifest.permissions,
        &granted_permissions.capabilities,
    )?;

    let permission_flags = resolve_capability_flags(&effective_capabilities, None)?;

    let (app_id, app_dir, existing_app) =
        source.resolve_target(&root, base_path, &prepared)?;

    let storage =
        resolve_storage_for_install(app, base_path, existing_app.as_ref()).await?;

    recreate_app_dir(&app_dir)?;

    let snapshot = source.create_snapshot(&app_dir, &prepared).await?;

    let origin_id = source.origin_id(base_path, &app_id, existing_app.as_ref())?;

    let installed = build_installed_app(
        app_id.clone(),
        origin_id,
        &app_dir,
        manifest,
        granted_permissions,
        permission_flags,
        storage,
        source.source(&prepared),
        snapshot,
    );

    write_installed_app_metadata(&installed, &app_dir)?;

    Ok(installed)
}

pub fn build_installed_app(
    app_id: String,
    origin_id: String,
    app_dir: &Path,
    manifest: &SageAppPackageManifest,
    granted_permissions: SageGrantedPermissions,
    permission_flags: SageAppCapabilityFlags,
    storage: InstalledSageAppStorage,
    source: UserSageAppSource,
    snapshot: SageAppSnapshot,
) -> UserSageApp {
    UserSageApp {
        common: SageAppCommon {
            id: app_id,
            origin_id,
            name: manifest.name.clone(),
            version: manifest.version.clone(),
            app_dir: app_dir.to_string_lossy().to_string(),
            entry_file: manifest_entry_file(manifest).to_string(),
            icon_file: manifest_icon_file(manifest).to_string(),
            requested_permissions: manifest.permissions.clone(),
            granted_permissions,
            capability_flags: permission_flags,
            storage,
            active_snapshot: snapshot,
        },
        source,
        pending_update: None,
    }
}

pub fn recreate_app_dir(app_dir: &Path) -> AnyResult<()> {
    if app_dir.exists() {
        fs::remove_dir_all(app_dir)?;
    }

    fs::create_dir_all(app_dir)?;

    Ok(())
}

async fn resolve_storage_for_install(
    app: &AppHandle,
    base_path: &Path,
    existing: Option<&UserSageApp>,
) -> AnyResult<InstalledSageAppStorage> {
    if let Some(existing) = existing {
        return Ok(existing.common.storage.clone());
    }

    allocate_new_storage(app, base_path).await
}
