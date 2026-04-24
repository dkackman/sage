use std::{
    collections::BTreeSet,
    io,
    path::{Path, PathBuf},
};

use anyhow::Result as AnyResult;
use tauri::{State, command};
use tauri::AppHandle;

use crate::bridge::{
    emit_granted_capabilities_change_for_app,
    emit_granted_network_whitelist_change_for_app,
};
use crate::bridge::capabilities::UserBridgeCapability;
use crate::host::{AppState, Result};
use crate::lifecycle::{
    download_url_snapshot, manifest_entry_file, manifest_icon_file,
    normalize_and_validate_granted_network_whitelist, preview_app_url_internal,
};
use crate::lifecycle::registry::{
    parse_network_permission_target, read_installed_app_by_id, write_installed_app_metadata,
};
use crate::permissions::{clear_storage_may_contain_secrets, mark_storage_may_contain_secrets, normalize_user_granted_capabilities, resolve_capability_flags, resolve_effective_granted_capabilities, validate_user_granted_capabilities};
use crate::types::{
    SageAppUrlPreview, SageGrantedNetworkPermissions, SageGrantedPermissions,
    SageNetworkPermissionTarget, UserSageApp, UserSageAppPendingUpdate,
    UserSageAppSource,
};

#[derive(Debug, Clone)]
pub struct GrantedCapabilitiesChange {
    pub removed: Vec<UserBridgeCapability>,
    pub added: Vec<UserBridgeCapability>,
    pub full: Vec<UserBridgeCapability>,
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
        capability: UserBridgeCapability,
        full_granted_capabilities: Vec<UserBridgeCapability>,
    },
    Granted {
        capability: UserBridgeCapability,
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

fn sort_unique_network(
    values: impl IntoIterator<Item = SageNetworkPermissionTarget>,
) -> Vec<SageNetworkPermissionTarget> {
    let set: BTreeSet<SageNetworkPermissionTarget> = values.into_iter().collect();
    set.into_iter().collect()
}

fn requested_capability_set(app: &UserSageApp) -> BTreeSet<UserBridgeCapability> {
    let mut out = BTreeSet::new();
    out.extend(app.common.requested_permissions.capabilities.required.iter().copied());
    out.extend(app.common.requested_permissions.capabilities.optional.iter().copied());
    out
}

fn requested_network_set(app: &UserSageApp) -> AnyResult<BTreeSet<(String, String)>> {
    let mut out = BTreeSet::new();

    for entry in &app.common.requested_permissions.network.whitelist.required {
        let normalized = parse_network_permission_target(&format!(
            "{}://{}",
            entry.scheme, entry.host
        ))
            .map_err(anyhow::Error::msg)?;
        out.insert((normalized.scheme, normalized.host));
    }

    for entry in &app.common.requested_permissions.network.whitelist.optional {
        let normalized = parse_network_permission_target(&format!(
            "{}://{}",
            entry.scheme, entry.host
        ))
            .map_err(anyhow::Error::msg)?;
        out.insert((normalized.scheme, normalized.host));
    }

    Ok(out)
}

fn diff_capabilities(
    previous: &[UserBridgeCapability],
    next: &[UserBridgeCapability],
) -> GrantedCapabilitiesChange {
    let previous_set: BTreeSet<UserBridgeCapability> =
        previous.iter().copied().collect();
    let next_set: BTreeSet<UserBridgeCapability> =
        next.iter().copied().collect();

    GrantedCapabilitiesChange {
        removed: previous_set.difference(&next_set).copied().collect(),
        added: next_set.difference(&previous_set).copied().collect(),
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

fn sort_unique_capabilities(
    capabilities: Vec<UserBridgeCapability>,
) -> Vec<UserBridgeCapability> {
    capabilities.into_iter().collect::<BTreeSet<_>>().into_iter().collect()
}

pub fn update_app_permissions_internal(
    base_path: &Path,
    app_id: &str,
    granted_permissions: SageGrantedPermissions,
    clear_storage_taint: bool,
) -> AnyResult<UserSageApp> {
    let mut app = read_installed_app_by_id(base_path, app_id)?;

    validate_user_granted_capabilities(
        &app.common.requested_permissions,
        &granted_permissions.capabilities,
    )?;

    let normalized_capabilities = normalize_user_granted_capabilities(
        &app.common.requested_permissions,
        &granted_permissions.capabilities,
    )?;

    let effective_capabilities = resolve_effective_granted_capabilities(
        &app.common.requested_permissions,
        &normalized_capabilities,
    )?;

    let granted_network_whitelist = normalize_and_validate_granted_network_whitelist(
        &app.common.active_snapshot.manifest.permissions.network,
        &granted_permissions.network.whitelist,
    )?;

    let mut permission_flags = resolve_capability_flags(
        &effective_capabilities,
        Some(&app.common.capability_flags),
    )?;

    if clear_storage_taint {
        permission_flags = clear_storage_may_contain_secrets(&permission_flags);
    }

    app.common.granted_permissions = SageGrantedPermissions {
        capabilities: normalized_capabilities,
        network: SageGrantedNetworkPermissions {
            whitelist: granted_network_whitelist,
        },
    };

    let effective_capabilities = resolve_effective_granted_capabilities(
        &app.common.requested_permissions,
        &app.common.granted_permissions.capabilities,
    )?;

    app.common.capability_flags = resolve_capability_flags(
        &effective_capabilities,
        Some(&permission_flags),
    )?;

    let app_dir = PathBuf::from(&app.common.app_dir);
    write_installed_app_metadata(&app, &app_dir)?;

    Ok(app)
}

pub fn grant_requested_capability_internal(
    base_path: &Path,
    app_id: &str,
    capability: UserBridgeCapability,
) -> AnyResult<GrantCapabilityOutcome> {
    let app = read_installed_app_by_id(base_path, app_id)?;

    let requested = requested_capability_set(&app);
    if !requested.contains(&capability) {
        anyhow::bail!(
            "Capability was not requested by app manifest: {}",
            capability.key()
        );
    }
    let definition = crate::permissions::require_user_capability_definition(capability)?;

    if !definition.flags.user_grantable {
        anyhow::bail!(
            "Capability is not user-grantable and cannot be persisted as a user grant: {}",
            capability.key()
        );
    }

    if app.common.granted_permissions.capabilities.contains(&capability) {
        let full_granted_capabilities =
            sort_unique_capabilities(app.common.granted_permissions.capabilities.clone());

        return Ok(GrantCapabilityOutcome::AlreadyGranted {
            capability,
            full_granted_capabilities,
        });
    }

    let mut next_capabilities = app.common.granted_permissions.capabilities.clone();
    next_capabilities.push(capability);
    next_capabilities = sort_unique_capabilities(next_capabilities);

    let updated = update_app_permissions_internal(
        base_path,
        app_id,
        SageGrantedPermissions {
            capabilities: next_capabilities,
            network: app.common.granted_permissions.network.clone(),
        },
        false,
    )?;

    let change = diff_capabilities(
        &app.common.granted_permissions.capabilities,
        &updated.common.granted_permissions.capabilities,
    );

    Ok(GrantCapabilityOutcome::Granted { capability, change })
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
        .common
        .granted_permissions
        .network
        .whitelist
        .iter()
        .any(|existing| existing == &entry)
    {
        let full_granted_network_whitelist =
            sort_unique_network(app.common.granted_permissions.network.whitelist.clone());

        return Ok(GrantNetworkWhitelistOutcome::AlreadyGranted {
            entry,
            full_granted_network_whitelist,
        });
    }

    let mut next_whitelist = app.common.granted_permissions.network.whitelist.clone();
    next_whitelist.push(entry.clone());
    next_whitelist = sort_unique_network(next_whitelist);

    let updated = update_app_permissions_internal(
        base_path,
        app_id,
        SageGrantedPermissions {
            capabilities: app.common.granted_permissions.capabilities.clone(),
            network: SageGrantedNetworkPermissions {
                whitelist: next_whitelist,
            },
        },
        false,
    )?;

    let change = diff_network_whitelist(
        &app.common.granted_permissions.network.whitelist,
        &updated.common.granted_permissions.network.whitelist,
    );

    Ok(GrantNetworkWhitelistOutcome::Granted { entry, change })
}

pub fn update_app_permissions_with_change_internal(
    base_path: &Path,
    app_id: &str,
    granted_permissions: SageGrantedPermissions,
    clear_storage_taint: bool,
) -> AnyResult<(
    UserSageApp,
    GrantedCapabilitiesChange,
    GrantedNetworkWhitelistChange,
)> {
    let previous = read_installed_app_by_id(base_path, app_id)?;

    let updated = update_app_permissions_internal(
        base_path,
        app_id,
        granted_permissions,
        clear_storage_taint,
    )?;

    let capability_change = diff_capabilities(
        &previous.common.granted_permissions.capabilities,
        &updated.common.granted_permissions.capabilities,
    );

    let network_change = diff_network_whitelist(
        &previous.common.granted_permissions.network.whitelist,
        &updated.common.granted_permissions.network.whitelist,
    );

    Ok((updated, capability_change, network_change))
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

    let app_url = match &app.source {
        UserSageAppSource::Url { app_url, .. } => app_url.clone(),
        UserSageAppSource::Zip => return Ok(None),
    };

    let preview = preview_app_url_internal(app_url)
        .await
        .map_err(|err| io::Error::other(format!("failed to preview app URL: {err}")))?;

    let same_manifest_hash =
        preview.manifest_hash == app.common.active_snapshot.manifest_hash;
    let same_manifest_content = preview.manifest == app.common.active_snapshot.manifest;

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
) -> Result<UserSageApp> {
    let base_path = {
        let state = state.lock().await;
        state.path.clone()
    };

    let mut app = read_installed_app_by_id(&base_path, &app_id).map_err(|err| {
        io::Error::other(format!("failed to read installed app {}: {err}", app_id))
    })?;

    let (app_url, manifest_url) = match &app.source {
        UserSageAppSource::Url {
            app_url,
            manifest_url,
        } => (app_url.clone(), manifest_url.clone()),
        UserSageAppSource::Zip => {
            return Err(
                io::Error::other("zip apps do not support URL update download").into(),
            );
        }
    };

    let preview = match check_app_update(state, app_id.clone()).await? {
        Some(preview) => preview,
        None => return Ok(app),
    };

    app.pending_update = Some(UserSageAppPendingUpdate {
        app_url,
        manifest_url,
        manifest_hash: preview.manifest_hash,
        manifest: preview.manifest,
    });

    let app_dir = PathBuf::from(&app.common.app_dir);
    write_installed_app_metadata(&app, &app_dir)
        .map_err(|err| io::Error::other(format!("failed to write app metadata: {err}")))?;

    Ok(app)
}

#[command]
#[specta::specta]
pub async fn apply_app_update(
    state: State<'_, AppState>,
    app_id: String,
    granted_permissions: SageGrantedPermissions,
) -> Result<UserSageApp> {
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

    validate_user_granted_capabilities(
        &pending.manifest.permissions,
        &granted_permissions.capabilities,
    )
        .map_err(|err| io::Error::other(format!("invalid granted permissions for update: {err}")))?;

    let normalized_capabilities = normalize_user_granted_capabilities(
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

    let effective_capabilities = resolve_effective_granted_capabilities(
        &pending.manifest.permissions,
        &normalized_capabilities,
    )
        .map_err(|err| {
            io::Error::other(format!("invalid granted permission policy for update: {err}"))
        })?;

    let permission_flags = resolve_capability_flags(
        &effective_capabilities,
        Some(&app.common.capability_flags),
    )
        .map_err(|err| {
            io::Error::other(format!("invalid granted permission policy for update: {err}"))
        })?;

    let app_dir = PathBuf::from(&app.common.app_dir);

    let snapshot = download_url_snapshot(
        &app_dir,
        &pending.app_url,
        &pending.manifest,
        &pending.manifest_hash,
    )
        .await
        .map_err(|err| io::Error::other(format!("failed to download update snapshot: {err}")))?;

    app.common.name = pending.manifest.name.clone();
    app.common.version = pending.manifest.version.clone();
    app.common.requested_permissions = pending.manifest.permissions.clone();

    app.common.granted_permissions = SageGrantedPermissions {
        capabilities: normalized_capabilities,
        network: SageGrantedNetworkPermissions {
            whitelist: granted_network_whitelist,
        },
    };

    app.common.capability_flags = permission_flags;
    app.common.active_snapshot = snapshot;
    app.common.entry_file =
        manifest_entry_file(&app.common.active_snapshot.manifest).to_string();
    app.common.icon_file =
        manifest_icon_file(&app.common.active_snapshot.manifest).to_string();
    app.pending_update = None;

    write_installed_app_metadata(&app, &app_dir)
        .map_err(|err| io::Error::other(format!("failed to write app metadata: {err}")))?;

    Ok(app)
}

#[command]
#[specta::specta]
pub async fn apps_update_permissions(
    app: AppHandle,
    state: State<'_, AppState>,
    app_id: String,
    granted_permissions: SageGrantedPermissions,
    clear_storage_taint: bool,
) -> Result<()> {
    let base_path = {
        let state = state.lock().await;
        state.path.clone()
    };

    let (_updated, capability_change, network_change) =
        update_app_permissions_with_change_internal(
            &base_path,
            &app_id,
            granted_permissions,
            clear_storage_taint,
        )
            .map_err(|err| io::Error::other(format!("failed to update app permissions: {err}")))?;

    if !capability_change.added.is_empty() || !capability_change.removed.is_empty() {
        let _ = emit_granted_capabilities_change_for_app(
            &app,
            &app_id,
            "sage-bridge",
            capability_change,
        )
            .await;
    }

    if !network_change.added.is_empty() || !network_change.removed.is_empty() {
        let _ = emit_granted_network_whitelist_change_for_app(
            &app,
            &app_id,
            "sage-bridge",
            network_change,
        )
            .await;
    }

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

    if !app.common.capability_flags.has_secret_access {
        return Ok(());
    }

    if app.common.capability_flags.storage_may_contain_secrets {
        return Ok(());
    }

    app.common.capability_flags =
        mark_storage_may_contain_secrets(&app.common.capability_flags);

    let app_dir = PathBuf::from(&app.common.app_dir);
    write_installed_app_metadata(&app, &app_dir)
        .map_err(|err| io::Error::other(format!("failed to write metadata: {err}")))?;

    Ok(())
}
