use std::{io, path::PathBuf};

use tauri::{command, State};

use crate::apps::install::{
    manifest_entry_file, manifest_icon_file,
    normalize_and_validate_granted_network_whitelist, preview_app_url_internal,
};

use crate::{
    app_state::AppState,
    apps::{
        permissions::{
            clear_storage_may_contain_secrets, mark_storage_may_contain_secrets,
            resolve_capability_flags, validate_granted_capabilities,
        },
        registry::{read_installed_app_by_id, write_installed_app_metadata},
        snapshot::download_url_snapshot,
        types::{
            InstalledSageApp, InstalledSageAppPendingUpdate, InstalledSageAppSource,
            SageAppUrlPreview,
        },
    },
    error::Result,
};
use crate::apps::types::SageGrantedPermissions;

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

    let app = read_installed_app_by_id(&base_path, &app_id).map_err(|err| {
        io::Error::other(format!("failed to read installed app {}: {err}", app_id))
    })?;

    let (app_url, _) = match &app.source {
        InstalledSageAppSource::Url {
            app_url,
            manifest_url: _,
        } => (app_url.clone(), true),
        InstalledSageAppSource::Zip => return Ok(None),
    };

    let preview = preview_app_url_internal(app_url)
        .await
        .map_err(|err| io::Error::other(format!("failed to preview app URL: {err}")))?;

    let same_manifest_hash = preview.manifest_hash == app.active_snapshot.manifest_hash;
    let same_manifest_content = preview.manifest == app.active_snapshot.manifest;

    if same_manifest_hash && same_manifest_content {
        return Ok(None);
    }

    if let Some(pending) = &app.pending_update {
        let same_pending_hash = pending.manifest_hash == preview.manifest_hash;
        let same_pending_manifest = pending.manifest == preview.manifest;

        if same_pending_hash && same_pending_manifest {
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

    let mut app = read_installed_app_by_id(&base_path, &app_id).map_err(|err| {
        io::Error::other(format!("failed to read installed app {}: {err}", app_id))
    })?;

    let (app_url, manifest_url) = match &app.source {
        InstalledSageAppSource::Url {
            app_url,
            manifest_url,
        } => (app_url.clone(), manifest_url.clone()),
        InstalledSageAppSource::Zip => {
            return Err(
                io::Error::other("zip apps do not support URL update download").into(),
            );
        }
    };

    let preview = match check_app_update(state, app_id.clone()).await? {
        Some(preview) => preview,
        None => return Ok(app),
    };

    app.pending_update = Some(InstalledSageAppPendingUpdate {
        app_url,
        manifest_url,
        manifest_hash: preview.manifest_hash,
        manifest: preview.manifest,
    });

    let install_dir = PathBuf::from(&app.install_dir);
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

    let mut app = read_installed_app_by_id(&base_path, &app_id).map_err(|err| {
        io::Error::other(format!("failed to read installed app {}: {err}", app_id))
    })?;

    let pending = app
        .pending_update
        .clone()
        .ok_or_else(|| io::Error::other(format!("app {} has no pending update", app_id)))?;

    validate_granted_capabilities(
        &pending.manifest.permissions,
        &granted_permissions.capabilities,
    )
        .map_err(|err| io::Error::other(format!("invalid granted permissions for update: {err}")))?;

    let granted_network_whitelist = normalize_and_validate_granted_network_whitelist(
        &pending.manifest.permissions.network,
        &granted_permissions.network.whitelist,
    )
        .map_err(|err| {
            io::Error::other(format!("invalid granted network whitelist for update: {err}"))
        })?;

    let permission_flags = resolve_capability_flags(
        &granted_permissions.capabilities,
        Some(&app.permission_flags),
    )
        .map_err(|err| {
            io::Error::other(format!("invalid granted permission policy for update: {err}"))
        })?;

    let install_dir = PathBuf::from(&app.install_dir);

    let snapshot = download_url_snapshot(
        &install_dir,
        &pending.app_url,
        &pending.manifest,
        &pending.manifest_hash,
    )
        .await
        .map_err(|err| io::Error::other(format!("failed to download update snapshot: {err}")))?;

    app.name = pending.manifest.name.clone();
    app.version = pending.manifest.version.clone();
    app.requested_permissions = pending.manifest.permissions.clone();

    app.granted_permissions = SageGrantedPermissions {
        capabilities: granted_permissions.capabilities,
        network: crate::apps::types::SageGrantedNetworkPermissions {
            whitelist: granted_network_whitelist,
        },
    };

    app.permission_flags = permission_flags;
    app.active_snapshot = snapshot;
    app.entry_file = manifest_entry_file(&app.active_snapshot.manifest).to_string();
    app.icon_file = manifest_icon_file(&app.active_snapshot.manifest).to_string();
    app.pending_update = None;

    write_installed_app_metadata(&app, &install_dir)
        .map_err(|err| io::Error::other(format!("failed to write app metadata: {err}")))?;

    Ok(app)
}

#[command]
#[specta::specta]
pub async fn apps_update_permissions(
    state: State<'_, AppState>,
    app_id: String,
    granted_permissions: SageGrantedPermissions,
    clear_storage_taint: bool,
) -> Result<()> {
    let base_path = {
        let state = state.lock().await;
        state.path.clone()
    };

    let mut app = read_installed_app_by_id(&base_path, &app_id)
        .map_err(|err| io::Error::other(format!("failed to read app {app_id}: {err}")))?;

    validate_granted_capabilities(&app.requested_permissions, &granted_permissions.capabilities)
        .map_err(|err| io::Error::other(format!("invalid granted permissions: {err}")))?;

    let granted_network_whitelist = normalize_and_validate_granted_network_whitelist(
        &app.active_snapshot.manifest.permissions.network,
        &granted_permissions.network.whitelist,
    )
        .map_err(|err| io::Error::other(format!("invalid granted network whitelist: {err}")))?;

    let mut permission_flags = resolve_capability_flags(
        &granted_permissions.capabilities,
        Some(&app.permission_flags),
    )
        .map_err(|err| io::Error::other(err.to_string()))?;

    if clear_storage_taint {
        permission_flags = clear_storage_may_contain_secrets(&permission_flags);
    }

    app.granted_permissions = SageGrantedPermissions {
        capabilities: granted_permissions.capabilities,
        network: crate::apps::types::SageGrantedNetworkPermissions {
            whitelist: granted_network_whitelist,
        },
    };

    app.permission_flags = resolve_capability_flags(
        &app.granted_permissions.capabilities,
        Some(&permission_flags),
    )
        .map_err(|err| io::Error::other(err.to_string()))?;

    let install_dir = PathBuf::from(&app.install_dir);
    write_installed_app_metadata(&app, &install_dir)
        .map_err(|err| io::Error::other(format!("failed to write metadata: {err}")))?;

    Ok(())
}

#[command]
#[specta::specta]
pub async fn apps_mark_storage_may_contain_secrets(
    state: State<'_, AppState>,
    app_id: String,
) -> Result<()> {
    let base_path = {
        let state = state.lock().await;
        state.path.clone()
    };

    let mut app = read_installed_app_by_id(&base_path, &app_id)
        .map_err(|err| io::Error::other(format!("failed to read app {app_id}: {err}")))?;

    if !app.permission_flags.has_secret_access {
        return Ok(());
    }

    if app.permission_flags.storage_may_contain_secrets {
        return Ok(());
    }

    app.permission_flags = mark_storage_may_contain_secrets(&app.permission_flags);

    let install_dir = PathBuf::from(&app.install_dir);
    write_installed_app_metadata(&app, &install_dir)
        .map_err(|err| io::Error::other(format!("failed to write metadata: {err}")))?;

    Ok(())
}
