use std::{io, path::{Path, PathBuf}, collections::BTreeSet};

use anyhow::Result as AnyResult;
use tauri::{State, command};

use crate::apps::lifecycle::{
    download_url_snapshot, manifest_entry_file, manifest_icon_file, normalize_and_validate_granted_network_whitelist,
    preview_app_url_internal,
};
use crate::apps::lifecycle::registry::{
    parse_network_permission_target, read_installed_app_by_id, write_installed_app_metadata,
};
use crate::apps::types::{
    InstalledSageApp, InstalledSageAppPendingUpdate, InstalledSageAppSource, SageAppUrlPreview,
    SageGrantedNetworkPermissions, SageGrantedPermissions, SageNetworkPermissionTarget,
};
use crate::apps::permissions::{
    clear_storage_may_contain_secrets, mark_storage_may_contain_secrets,
    resolve_capability_flags, validate_granted_capabilities,
};
use crate::{app_state::AppState, error::Result};

#[derive(Debug, Clone)]
pub struct GrantedCapabilitiesChange {
    pub removed: Vec<String>,
    pub added: Vec<String>,
    pub full: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct GrantedNetworkWhitelistChange {
    pub removed: Vec<SageNetworkPermissionTarget>,
    pub added: Vec<SageNetworkPermissionTarget>,
    pub full: Vec<SageNetworkPermissionTarget>,
}

#[derive(Debug, Clone)]
pub enum GrantCapabilityOutcome {
    AlreadyGranted {
        capability: String,
        full_granted_capabilities: Vec<String>,
    },
    Granted {
        capability: String,
        change: GrantedCapabilitiesChange,
    },
}

#[derive(Debug, Clone)]
pub enum GrantNetworkWhitelistOutcome {
    AlreadyGranted {
        entry: SageNetworkPermissionTarget,
        full_granted_network_whitelist: Vec<SageNetworkPermissionTarget>,
    },
    Granted {
        entry: SageNetworkPermissionTarget,
        change: GrantedNetworkWhitelistChange,
    },
}

fn sort_unique_strings(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let set: BTreeSet<String> = values.into_iter().collect();
    set.into_iter().collect()
}

fn sort_unique_network(
    values: impl IntoIterator<Item = SageNetworkPermissionTarget>,
) -> Vec<SageNetworkPermissionTarget> {
    let set: BTreeSet<SageNetworkPermissionTarget> = values.into_iter().collect();
    set.into_iter().collect()
}

fn requested_capability_set(app: &InstalledSageApp) -> BTreeSet<String> {
    let mut requested = BTreeSet::new();
    requested.extend(app.requested_permissions.capabilities.required.iter().cloned());
    requested.extend(app.requested_permissions.capabilities.optional.iter().cloned());
    requested
}

fn requested_network_set(app: &InstalledSageApp) -> AnyResult<BTreeSet<(String, String)>> {
    let mut out = BTreeSet::new();

    for entry in &app.requested_permissions.network.whitelist.required {
        let normalized = parse_network_permission_target(&format!(
            "{}://{}",
            entry.scheme, entry.host
        ))
            .map_err(anyhow::Error::msg)?;
        out.insert((normalized.scheme, normalized.host));
    }

    for entry in &app.requested_permissions.network.whitelist.optional {
        let normalized = parse_network_permission_target(&format!(
            "{}://{}",
            entry.scheme, entry.host
        ))
            .map_err(anyhow::Error::msg)?;
        out.insert((normalized.scheme, normalized.host));
    }

    Ok(out)
}

fn diff_capabilities(previous: &[String], next: &[String]) -> GrantedCapabilitiesChange {
    let previous_set: BTreeSet<String> = previous.iter().cloned().collect();
    let next_set: BTreeSet<String> = next.iter().cloned().collect();

    GrantedCapabilitiesChange {
        removed: previous_set.difference(&next_set).cloned().collect(),
        added: next_set.difference(&previous_set).cloned().collect(),
        full: next.to_vec(),
    }
}

fn diff_network_whitelist(
    previous: &[SageNetworkPermissionTarget],
    next: &[SageNetworkPermissionTarget],
) -> GrantedNetworkWhitelistChange {
    let previous_set: BTreeSet<SageNetworkPermissionTarget> = previous.iter().cloned().collect();
    let next_set: BTreeSet<SageNetworkPermissionTarget> = next.iter().cloned().collect();

    GrantedNetworkWhitelistChange {
        removed: previous_set.difference(&next_set).cloned().collect(),
        added: next_set.difference(&previous_set).cloned().collect(),
        full: next.to_vec(),
    }
}

pub fn update_app_permissions_internal(
    base_path: &Path,
    app_id: &str,
    granted_permissions: SageGrantedPermissions,
    clear_storage_taint: bool,
) -> AnyResult<InstalledSageApp> {
    let mut app = read_installed_app_by_id(base_path, app_id)?;

    validate_granted_capabilities(&app.requested_permissions, &granted_permissions.capabilities)?;

    let granted_network_whitelist = normalize_and_validate_granted_network_whitelist(
        &app.active_snapshot.manifest.permissions.network,
        &granted_permissions.network.whitelist,
    )?;

    let mut permission_flags = resolve_capability_flags(
        &granted_permissions.capabilities,
        Some(&app.capability_flags),
    )?;

    if clear_storage_taint {
        permission_flags = clear_storage_may_contain_secrets(&permission_flags);
    }

    app.granted_permissions = SageGrantedPermissions {
        capabilities: granted_permissions.capabilities,
        network: SageGrantedNetworkPermissions {
            whitelist: granted_network_whitelist,
        },
    };

    app.capability_flags =
        resolve_capability_flags(&app.granted_permissions.capabilities, Some(&permission_flags))?;

    let install_dir = PathBuf::from(&app.install_dir);
    write_installed_app_metadata(&app, &install_dir)?;

    Ok(app)
}

pub fn grant_requested_capability_internal(
    base_path: &Path,
    app_id: &str,
    capability: &str,
) -> AnyResult<GrantCapabilityOutcome> {
    let app = read_installed_app_by_id(base_path, app_id)?;

    let requested = requested_capability_set(&app);
    if !requested.contains(capability) {
        anyhow::bail!("Capability was not requested by app manifest: {capability}");
    }

    if app
        .granted_permissions
        .capabilities
        .iter()
        .any(|existing| existing == capability)
    {
        let full_granted_capabilities =
            sort_unique_strings(app.granted_permissions.capabilities.clone());

        return Ok(GrantCapabilityOutcome::AlreadyGranted {
            capability: capability.to_string(),
            full_granted_capabilities,
        });
    }

    let mut next_capabilities = app.granted_permissions.capabilities.clone();
    next_capabilities.push(capability.to_string());
    next_capabilities = sort_unique_strings(next_capabilities);

    let updated = update_app_permissions_internal(
        base_path,
        app_id,
        SageGrantedPermissions {
            capabilities: next_capabilities,
            network: app.granted_permissions.network.clone(),
        },
        false,
    )?;

    let change = diff_capabilities(
        &app.granted_permissions.capabilities,
        &updated.granted_permissions.capabilities,
    );

    Ok(GrantCapabilityOutcome::Granted {
        capability: capability.to_string(),
        change,
    })
}

pub fn grant_requested_network_whitelist_entry_internal(
    base_path: &Path,
    app_id: &str,
    entry: &SageNetworkPermissionTarget,
) -> AnyResult<GrantNetworkWhitelistOutcome> {
    let app = read_installed_app_by_id(base_path, app_id)?;

    let normalized = parse_network_permission_target(&format!(
        "{}://{}",
        entry.scheme, entry.host
    ))
        .map_err(anyhow::Error::msg)?;
    let entry = SageNetworkPermissionTarget {
        scheme: normalized.scheme,
        host: normalized.host,
    };

    let requested = requested_network_set(&app)?;
    let entry_key = (entry.scheme.clone(), entry.host.clone());

    if !requested.contains(&entry_key) {
        anyhow::bail!(
            "Network whitelist entry was not requested by app manifest: {}://{}",
            entry.scheme,
            entry.host
        );
    }

    if app
        .granted_permissions
        .network
        .whitelist
        .iter()
        .any(|existing| existing == &entry)
    {
        let full_granted_network_whitelist =
            sort_unique_network(app.granted_permissions.network.whitelist.clone());

        return Ok(GrantNetworkWhitelistOutcome::AlreadyGranted {
            entry,
            full_granted_network_whitelist,
        });
    }

    let mut next_whitelist = app.granted_permissions.network.whitelist.clone();
    next_whitelist.push(entry.clone());
    next_whitelist = sort_unique_network(next_whitelist);

    let updated = update_app_permissions_internal(
        base_path,
        app_id,
        SageGrantedPermissions {
            capabilities: app.granted_permissions.capabilities.clone(),
            network: SageGrantedNetworkPermissions {
                whitelist: next_whitelist,
            },
        },
        false,
    )?;

    let change = diff_network_whitelist(
        &app.granted_permissions.network.whitelist,
        &updated.granted_permissions.network.whitelist,
    );

    Ok(GrantNetworkWhitelistOutcome::Granted { entry, change })
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
        Some(&app.capability_flags),
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
        network: SageGrantedNetworkPermissions {
            whitelist: granted_network_whitelist,
        },
    };

    app.capability_flags = permission_flags;
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

    update_app_permissions_internal(
        &base_path,
        &app_id,
        granted_permissions,
        clear_storage_taint,
    )
        .map(|_| ())
        .map_err(|err| io::Error::other(format!("failed to update app permissions: {err}")).into())
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

    if !app.capability_flags.has_secret_access {
        return Ok(());
    }

    if app.capability_flags.storage_may_contain_secrets {
        return Ok(());
    }

    app.capability_flags = mark_storage_may_contain_secrets(&app.capability_flags);

    let install_dir = PathBuf::from(&app.install_dir);
    write_installed_app_metadata(&app, &install_dir)
        .map_err(|err| io::Error::other(format!("failed to write metadata: {err}")))?;

    Ok(())
}
