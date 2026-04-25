use std::{fs, io, path::Path};

use tauri::{command, AppHandle, State};

use crate::host::{AppState, Result};
use crate::lifecycle::{apps_clear_runtime_browsing_data, apps_root, enqueue_pending_storage_cleanup, enqueue_retired_app_origin, install_app_from_source, list_installed_apps_internal, read_manifest, unzip_to_dir, validate_package_structure};
use crate::lifecycle::registry::read_installed_app_by_id;
use crate::permissions::normalize_and_validate_requested_permissions;
use crate::types::{
    ListedSageApp, SageAppPackageManifest, SageAppUrlPreview, SageGrantedPermissions, UserSageApp,
};
use uuid::Uuid;
use crate::lifecycle::url::{preview_app_url_internal, UrlInstallSource};
use crate::lifecycle::zip::ZipInstallSource;

#[command]
#[specta::specta]
pub async fn preview_app_zip(zip_path: String) -> Result<SageAppPackageManifest> {
    let unpack_dir = std::env::temp_dir().join(format!(".sage-preview-{}", Uuid::new_v4()));

    let result = (|| -> anyhow::Result<SageAppPackageManifest> {
        unzip_to_dir(Path::new(&zip_path), &unpack_dir)?;
        let package_root = crate::lifecycle::detect_package_root(&unpack_dir)?;
        let manifest = read_manifest(&package_root)?;
        validate_package_structure(&package_root)?;

        let mut manifest = manifest;
        manifest.permissions =
            normalize_and_validate_requested_permissions(&manifest.permissions)?;

        Ok(manifest)
    })();

    let _ = fs::remove_dir_all(&unpack_dir);

    result.map_err(|err| {
        io::Error::other(format!("failed to preview app zip {}: {err}", zip_path)).into()
    })
}

#[command]
#[specta::specta]
pub async fn preview_app_url(app_url: String) -> Result<SageAppUrlPreview> {
    preview_app_url_internal(app_url)
        .await
        .map_err(|err| io::Error::other(format!("failed to preview app URL: {err}")).into())
}

#[command]
#[specta::specta]
pub async fn list_installed_apps(state: State<'_, AppState>) -> Result<Vec<ListedSageApp>> {
    let base_path = {
        let state = state.lock().await;
        state.path.clone()
    };

    let root = apps_root(&base_path);

    fs::create_dir_all(&root).map_err(|err| {
        io::Error::other(format!("failed to create apps directory {}: {err}", root.display()))
    })?;

    list_installed_apps_internal(&root)
        .map_err(|err| io::Error::other(format!("failed to list installed apps: {err}")).into())
}

#[command]
#[specta::specta]
pub async fn install_app_zip(
    app: AppHandle,
    state: State<'_, AppState>,
    zip_path: String,
    granted_permissions: SageGrantedPermissions,
) -> Result<UserSageApp> {
    let base_path = {
        let state = state.lock().await;
        state.path.clone()
    };

    let root = apps_root(&base_path);

    fs::create_dir_all(&root).map_err(|err| {
        io::Error::other(format!("failed to create apps directory {}: {err}", root.display()))
    })?;

    let source = ZipInstallSource::new(&root, zip_path.clone());
    let unpack_dir = source.unpack_dir.clone();

    let result = install_app_from_source(
        &app,
        &base_path,
        granted_permissions,
        source,
    )
        .await;

    if unpack_dir.exists() {
        let _ = fs::remove_dir_all(&unpack_dir);
    }

    result.map_err(|err| {
        io::Error::other(format!("failed to install app zip {}: {err}", zip_path)).into()
    })
}

#[command]
#[specta::specta]
pub async fn install_app_url(
    app: AppHandle,
    state: State<'_, AppState>,
    app_url: String,
    granted_permissions: SageGrantedPermissions,
) -> Result<UserSageApp> {
    let base_path = {
        let state = state.lock().await;
        state.path.clone()
    };

    install_app_from_source(
        &app,
        &base_path,
        granted_permissions,
        UrlInstallSource {
            app_url: app_url.clone(),
        },
    )
        .await
        .map_err(|err| {
            io::Error::other(format!("failed to install app URL {}: {err}", app_url)).into()
        })
}

#[command]
#[specta::specta]
pub async fn uninstall_app(
    app: AppHandle,
    state: State<'_, AppState>,
    app_id: String,
) -> Result<()> {
    let base_path = {
        let state = state.lock().await;
        state.path.clone()
    };

    let installed = read_installed_app_by_id(&base_path, &app_id).ok();

    if let Some(installed) = &installed {
        let cleanup_result = apps_clear_runtime_browsing_data(app.clone(), app_id.clone()).await;

        match cleanup_result {
            Ok(()) => {
                enqueue_retired_app_origin(&base_path, installed, false).map_err(|err| {
                    io::Error::other(format!(
                        "failed to retire app origin after uninstall cleanup: {err}"
                    ))
                })?;
            }
            Err(err) => {
                enqueue_pending_storage_cleanup(&base_path, installed, &err).map_err(
                    |queue_err| {
                        io::Error::other(format!(
                            "failed to enqueue pending storage cleanup after clear failure ({err}): {queue_err}"
                        ))
                    },
                )?;

                enqueue_retired_app_origin(&base_path, installed, true).map_err(|origin_err| {
                    io::Error::other(format!(
                        "failed to retire app origin after cleanup failure ({err}): {origin_err}"
                    ))
                })?;
            }
        }
    }

    let dir = apps_root(&base_path).join(&app_id);

    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|err| {
            io::Error::other(format!(
                "failed to remove installed app {} at {}: {err}",
                app_id,
                dir.display()
            ))
        })?;
    }

    Ok(())
}
