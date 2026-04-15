use std::{io, path::{Path, PathBuf}};

use tauri::{command, State};

use crate::{
    app_state::AppState,
    apps::{
        permissions::validate_granted_permissions_against_requested,
        registry::{read_installed_app_by_id, write_installed_app_metadata},
        snapshot::download_url_snapshot,
        types::{
            InstalledSageApp, InstalledSageAppPendingUpdate, InstalledSageAppSource,
            SageAppUrlPreview, SageGrantedPermissions, SageGrantedNetworkPermissionEntry,
            SageRequestedPermissions,
        },
    },
    error::Result,
};

use super::install::preview_app_url;

fn merge_granted_permissions_for_update(
    old_granted: &SageGrantedPermissions,
    new_requested: &SageRequestedPermissions,
    user_selected: &SageGrantedPermissions,
) -> SageGrantedPermissions {
    let requested_network: std::collections::BTreeSet<(String, String)> = new_requested
        .network
        .iter()
        .map(|entry| (entry.scheme.to_ascii_lowercase(), entry.host.to_ascii_lowercase()))
        .collect();

    let old_network: Vec<SageGrantedNetworkPermissionEntry> = old_granted
        .network
        .iter()
        .filter(|entry| {
            requested_network.contains(&(entry.scheme.to_ascii_lowercase(), entry.host.to_ascii_lowercase()))
        })
        .cloned()
        .collect();

    let mut merged_network = old_network;

    for entry in &user_selected.network {
        let key = (entry.scheme.to_ascii_lowercase(), entry.host.to_ascii_lowercase());
        let already = merged_network.iter().any(|existing| {
            existing.scheme.eq_ignore_ascii_case(&entry.scheme)
                && existing.host.eq_ignore_ascii_case(&entry.host)
        });

        if requested_network.contains(&key) && !already {
            merged_network.push(entry.clone());
        }
    }

    SageGrantedPermissions {
        network: merged_network,
        persistent_storage: new_requested.persistent_storage.is_some()
            && (old_granted.persistent_storage || user_selected.persistent_storage),
    }
}

#[command]
#[specta::specta]
pub async fn check_app_update(
    state: State<'_, AppState>,
    app_id: String,
) -> Result<Option<SageAppUrlPreview>> {
    let base_path = {
        let state = state.lock().await;
        state.path.clone()
    };

    let app = read_installed_app_by_id(&base_path, &app_id)
        .map_err(|err| io::Error::other(format!("failed to read installed app {}: {err}", app_id)))?;

    let (app_url, _manifest_url) = match &app.source {
        InstalledSageAppSource::Url { app_url, manifest_url } => {
            (app_url.clone(), manifest_url.clone())
        }
        InstalledSageAppSource::Zip => return Ok(None),
    };

    let preview = preview_app_url(app_url).await?;

    if preview.manifest_hash == app.active_snapshot.manifest_hash {
        return Ok(None);
    }

    if let Some(pending) = &app.pending_update {
        if pending.manifest_hash == preview.manifest_hash {
            return Ok(None);
        }
    }

    Ok(Some(preview))
}

#[command]
#[specta::specta]
pub async fn download_app_update(
    state: State<'_, AppState>,
    app_id: String,
) -> Result<InstalledSageApp> {
    let base_path = {
        let state = state.lock().await;
        state.path.clone()
    };

    let mut app = read_installed_app_by_id(&base_path, &app_id)
        .map_err(|err| io::Error::other(format!("failed to read installed app {}: {err}", app_id)))?;

    let (app_url, manifest_url) = match &app.source {
        InstalledSageAppSource::Url { app_url, manifest_url } => {
            (app_url.clone(), manifest_url.clone())
        }
        InstalledSageAppSource::Zip => {
            return Err(io::Error::other("zip apps do not support URL update download").into());
        }
    };

    let preview = match check_app_update(state, app_id.clone()).await? {
        Some(preview) => preview,
        None => return Ok(app),
    };

    let install_dir = PathBuf::from(&app.install_dir);

    let snapshot = download_url_snapshot(
        &install_dir,
        &preview.app_url,
        &preview.manifest,
        &preview.manifest_hash,
    )
        .await
        .map_err(|err| io::Error::other(format!("failed to download update snapshot: {err}")))?;

    app.pending_update = Some(InstalledSageAppPendingUpdate {
        app_url,
        manifest_url,
        manifest_hash: preview.manifest_hash,
        manifest: preview.manifest,
        snapshot,
    });

    write_installed_app_metadata(&app, &install_dir)
        .map_err(|err| io::Error::other(format!("failed to write app metadata: {err}")))?;

    Ok(app)
}

#[command]
#[specta::specta]
pub async fn apply_app_update(
    state: State<'_, AppState>,
    app_id: String,
    granted_permissions: SageGrantedPermissions,
) -> Result<InstalledSageApp> {
    let base_path = {
        let state = state.lock().await;
        state.path.clone()
    };

    let mut app = read_installed_app_by_id(&base_path, &app_id)
        .map_err(|err| io::Error::other(format!("failed to read installed app {}: {err}", app_id)))?;

    let pending = app.pending_update.clone().ok_or_else(|| {
        io::Error::other(format!("app {} has no pending update", app_id))
    })?;

    let merged_permissions = merge_granted_permissions_for_update(
        &app.granted_permissions,
        &pending.manifest.permissions,
        &granted_permissions,
    );

    validate_granted_permissions_against_requested(
        &pending.manifest.permissions,
        &merged_permissions,
    )
        .map_err(|err| io::Error::other(format!("invalid granted permissions for update: {err}")))?;

    app.name = pending.manifest.name.clone();
    app.version = pending.manifest.version.clone();
    app.requested_permissions = pending.manifest.permissions.clone();
    app.granted_permissions = merged_permissions;
    app.active_snapshot = pending.snapshot.clone();
    app.entry_file = Path::new(&app.active_snapshot.snapshot_dir)
        .join("index.html")
        .to_string_lossy()
        .to_string();
    app.icon_file = Path::new(&app.active_snapshot.snapshot_dir)
        .join("icon.png")
        .to_string_lossy()
        .to_string();
    app.pending_update = None;

    let install_dir = PathBuf::from(&app.install_dir);
    write_installed_app_metadata(&app, &install_dir)
        .map_err(|err| io::Error::other(format!("failed to write app metadata: {err}")))?;

    Ok(app)
}
