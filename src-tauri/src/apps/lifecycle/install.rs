use std::{
    collections::BTreeSet,
    fs, io,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result as AnyResult, anyhow};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, State, command};

use crate::app_state::AppState;
use crate::apps::lifecycle::{
    derive_manifest_url, download_url_snapshot, enqueue_pending_storage_cleanup,
    fetch_url_manifest, list_installed_apps_internal, prepare_zip_snapshot, read_manifest,
    retry_pending_storage_cleanup, unzip_to_dir, validate_package_structure,
    write_installed_app_metadata,
};
use crate::apps::lifecycle::registry::{apps_root, read_installed_app_by_id};
use crate::apps::permissions::{
    normalize_and_validate_requested_permissions, resolve_capability_flags,
    validate_granted_capabilities,
};
use crate::apps::runtime::apps_clear_runtime_browsing_data;
use crate::apps::types::{
    InstalledSageApp, InstalledSageAppSource, InstalledSageAppStorage, ListedSageApp,
    SageAppPackageManifest, SageAppUrlPreview,
};
use crate::apps::types::{
    SageGrantedPermissions, SageNetworkPermissionTarget, SageRequestedNetworkPermissions,
    SageRequestedPermissions,
};
use crate::error::Result;

pub fn current_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_millis()
}

fn slugify_name(name: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;

    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }

    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "app".to_string()
    } else {
        out
    }
}

fn generate_zip_app_id(name: &str) -> String {
    format!("{}-{}", slugify_name(name), current_millis())
}

fn generate_url_app_id(manifest_url: &str) -> String {
    let hash = hash_string(manifest_url);
    format!("url-{}", &hash[..16])
}

pub fn normalize_app_url(url: &str) -> AnyResult<String> {
    let mut parsed = reqwest::Url::parse(url).context("invalid app URL")?;

    let scheme = parsed.scheme();
    let host = parsed
        .host_str()
        .ok_or_else(|| anyhow!("app URL is missing host"))?
        .to_ascii_lowercase();

    let is_local_dev_host = host == "localhost" || host == "127.0.0.1" || host == "::1";

    match scheme {
        "https" => {}
        "http" if is_local_dev_host => {}
        "http" => {
            return Err(anyhow!(
                "URL app install requires HTTPS, except for localhost/127.0.0.1 development URLs"
            ));
        }
        other => {
            return Err(anyhow!("unsupported app URL scheme: {other}"));
        }
    }

    parsed.set_fragment(None);

    let path = parsed.path();
    if !path.ends_with('/') {
        parsed.set_path(&format!("{path}/"));
    }

    if parsed.query().is_some() {
        parsed.set_query(None);
    }

    Ok(parsed.to_string())
}

fn find_existing_installed_app_by_name(
    root: &Path,
    app_name: &str,
) -> AnyResult<Option<InstalledSageApp>> {
    Ok(list_installed_apps_internal(root)?
        .into_iter()
        .find_map(|app| match app {
            ListedSageApp::Installed(installed) if installed.name == app_name => {
                Some(installed)
            }
            _ => None,
        }))
}

fn build_installed_app(
    app_id: String,
    install_dir: &Path,
    manifest: &SageAppPackageManifest,
    granted_permissions: SageGrantedPermissions,
    permission_flags: crate::apps::types::InstalledSageAppCapabilityFlags,
    storage: InstalledSageAppStorage,
    source: InstalledSageAppSource,
    snapshot: crate::apps::types::InstalledSageAppSnapshot,
) -> InstalledSageApp {
    InstalledSageApp {
        id: app_id,
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        install_dir: install_dir.to_string_lossy().to_string(),
        entry_file: manifest_entry_file(manifest).to_string(),
        icon_file: manifest_icon_file(manifest).to_string(),
        requested_permissions: manifest.permissions.clone(),
        granted_permissions,
        capability_flags: permission_flags,
        storage,
        source,
        active_snapshot: snapshot,
        pending_update: None,
    }
}

fn recreate_install_dir(install_dir: &Path) -> AnyResult<()> {
    if install_dir.exists() {
        fs::remove_dir_all(install_dir).with_context(|| {
            format!("failed to remove existing install dir {}", install_dir.display())
        })?;
    }

    fs::create_dir_all(install_dir)
        .with_context(|| format!("failed to create install dir {}", install_dir.display()))?;

    Ok(())
}

fn resolve_zip_install_target(
    root: &Path,
    app_name: &str,
) -> AnyResult<(String, std::path::PathBuf, Option<InstalledSageApp>)> {
    if let Some(existing) = find_existing_installed_app_by_name(root, app_name)? {
        let install_dir = Path::new(&existing.install_dir).to_path_buf();
        return Ok((existing.id.clone(), install_dir, Some(existing)));
    }

    let app_id = generate_zip_app_id(app_name);
    Ok((app_id.clone(), root.join(&app_id), None))
}

fn resolve_url_install_target(
    root: &Path,
    manifest_url: &str,
) -> AnyResult<(String, std::path::PathBuf, Option<InstalledSageApp>)> {
    let app_id = generate_url_app_id(manifest_url);
    let install_dir = root.join(&app_id);

    let existing = if install_dir.exists() {
        Some(read_installed_app_by_id(root.parent().unwrap_or(root), &app_id)?)
    } else {
        None
    };

    Ok((app_id.clone(), install_dir, existing))
}

pub fn hash_string(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

fn normalize_manifest_permissions(
    mut manifest: SageAppPackageManifest,
) -> AnyResult<SageAppPackageManifest> {
    manifest.permissions = normalize_and_validate_requested_permissions(&manifest.permissions)?;
    Ok(manifest)
}

pub fn manifest_entry_file(manifest: &SageAppPackageManifest) -> &str {
    manifest.entry.as_deref().unwrap_or("index.html")
}

pub fn manifest_icon_file(manifest: &SageAppPackageManifest) -> &str {
    manifest.icon.as_deref().unwrap_or("icon.png")
}

fn normalize_and_validate_granted_permissions(
    requested: &SageRequestedPermissions,
    granted: SageGrantedPermissions,
) -> AnyResult<SageGrantedPermissions> {
    validate_granted_capabilities(requested, &granted.capabilities)?;

    let whitelist = normalize_and_validate_granted_network_whitelist(
        &requested.network,
        &granted.network.whitelist,
    )?;

    Ok(SageGrantedPermissions {
        capabilities: granted.capabilities,
        network: crate::apps::types::SageGrantedNetworkPermissions { whitelist },
    })
}

pub fn normalize_and_validate_granted_network_whitelist(
    requested: &SageRequestedNetworkPermissions,
    granted: &[SageNetworkPermissionTarget],
) -> AnyResult<Vec<SageNetworkPermissionTarget>> {
    let mut requested_required = BTreeSet::<(String, String)>::new();
    let mut requested_optional = BTreeSet::<(String, String)>::new();

    for entry in &requested.whitelist.required {
        requested_required.insert(crate::apps::permissions::normalize_network_key(
            &entry.scheme,
            &entry.host,
        )?);
    }

    for entry in &requested.whitelist.optional {
        let key = crate::apps::permissions::normalize_network_key(&entry.scheme, &entry.host)?;
        if !requested_required.contains(&key) {
            requested_optional.insert(key);
        }
    }

    let mut granted_keys = BTreeSet::<(String, String)>::new();

    for entry in granted {
        let key = crate::apps::permissions::normalize_network_key(&entry.scheme, &entry.host)?;
        if !requested_required.contains(&key) && !requested_optional.contains(&key) {
            return Err(anyhow!(
                "granted network whitelist entry not requested in manifest: {}://{}",
                key.0,
                key.1
            ));
        }
        granted_keys.insert(key);
    }

    let mut result = BTreeSet::<SageNetworkPermissionTarget>::new();

    for (scheme, host) in &requested_required {
        result.insert(SageNetworkPermissionTarget {
            scheme: scheme.clone(),
            host: host.clone(),
        });
    }

    for (scheme, host) in granted_keys {
        result.insert(SageNetworkPermissionTarget { scheme, host });
    }

    Ok(result.into_iter().collect())
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
async fn allocate_new_storage(
    app: &AppHandle,
    _base_path: &Path,
) -> AnyResult<InstalledSageAppStorage> {
    loop {
        let identifier = *uuid::Uuid::new_v4().as_bytes();
        let existing_ids = app
            .fetch_data_store_identifiers()
            .await
            .map_err(|err| anyhow!("failed to fetch data store identifiers: {err}"))?;

        if existing_ids.iter().all(|existing| *existing != identifier) {
            return Ok(InstalledSageAppStorage::AppleDataStore {
                identifier_hex: hex::encode(identifier),
            });
        }
    }
}

#[cfg(target_os = "windows")]
async fn allocate_new_storage(
    _app: &AppHandle,
    base_path: &Path,
) -> AnyResult<InstalledSageAppStorage> {
    let profiles_root = base_path.join("profiles");
    fs::create_dir_all(&profiles_root)
        .with_context(|| format!("failed to create profiles directory {}", profiles_root.display()))?;

    loop {
        let directory_name = format!("profile-{}", uuid::Uuid::new_v4());
        let candidate = profiles_root.join(&directory_name);

        if !candidate.exists() {
            return Ok(InstalledSageAppStorage::WindowsProfile { directory_name });
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "windows")))]
async fn allocate_new_storage(
    _app: &AppHandle,
    _base_path: &Path,
) -> AnyResult<InstalledSageAppStorage> {
    Ok(InstalledSageAppStorage::Unsupported)
}

async fn resolve_storage_for_install(
    app: &AppHandle,
    base_path: &Path,
    existing: Option<&InstalledSageApp>,
) -> AnyResult<InstalledSageAppStorage> {
    if let Some(existing) = existing {
        return Ok(existing.storage.clone());
    }

    allocate_new_storage(app, base_path).await
}

pub async fn preview_app_url_internal(app_url: String) -> AnyResult<SageAppUrlPreview> {
    let app_url = normalize_app_url(&app_url)?;

    let manifest_url = derive_manifest_url(&app_url)?;
    let (manifest, manifest_hash) = fetch_url_manifest(&manifest_url).await?;
    let manifest = normalize_manifest_permissions(manifest)?;

    Ok(SageAppUrlPreview {
        app_url,
        manifest_url,
        manifest_hash,
        manifest,
    })
}

#[command]
#[specta::specta]
pub async fn preview_app_zip(zip_path: String) -> Result<SageAppPackageManifest> {
    let unpack_dir = std::env::temp_dir().join(format!(".sage-preview-{}", current_millis()));

    let result = (|| -> AnyResult<SageAppPackageManifest> {
        unzip_to_dir(Path::new(&zip_path), &unpack_dir)?;
        let package_root = crate::apps::lifecycle::detect_package_root(&unpack_dir)?;
        let manifest = read_manifest(&package_root)?;
        validate_package_structure(&package_root)?;
        normalize_manifest_permissions(manifest)
    })();

    let _ = fs::remove_dir_all(&unpack_dir);

    result.map_err(|err| io::Error::other(format!("failed to preview app zip {}: {err}", zip_path)).into())
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
) -> Result<InstalledSageApp> {
    let base_path = {
        let state = state.lock().await;
        state.path.clone()
    };

    let root = apps_root(&base_path);

    fs::create_dir_all(&root).map_err(|err| {
        io::Error::other(format!("failed to create apps directory {}: {err}", root.display()))
    })?;

    let unpack_dir = root.join(format!(".tmp-{}", current_millis()));
    if unpack_dir.exists() {
        fs::remove_dir_all(&unpack_dir).map_err(|err| {
            io::Error::other(format!(
                "failed to remove temporary unpack directory {}: {err}",
                unpack_dir.display()
            ))
        })?;
    }

    let result: AnyResult<InstalledSageApp> = async {
        unzip_to_dir(Path::new(&zip_path), &unpack_dir)?;
        let package_root = crate::apps::lifecycle::detect_package_root(&unpack_dir)?;
        validate_package_structure(&package_root)?;

        let manifest = normalize_manifest_permissions(read_manifest(&package_root)?)?;

        let granted_permissions =
            normalize_and_validate_granted_permissions(&manifest.permissions, granted_permissions)?;

        let permission_flags = resolve_capability_flags(&granted_permissions.capabilities, None)?;

        let (app_id, install_dir, existing_app) = resolve_zip_install_target(&root, &manifest.name)?;
        let storage = resolve_storage_for_install(&app, &base_path, existing_app.as_ref()).await?;

        recreate_install_dir(&install_dir)?;
        let snapshot = prepare_zip_snapshot(&package_root, &install_dir, &manifest)?;

        let installed = build_installed_app(
            app_id,
            &install_dir,
            &manifest,
            granted_permissions,
            permission_flags,
            storage,
            InstalledSageAppSource::Zip,
            snapshot,
        );

        write_installed_app_metadata(&installed, &install_dir)?;
        Ok(installed)
    }
    .await;

    if unpack_dir.exists() {
        let _ = fs::remove_dir_all(&unpack_dir);
    }

    result.map_err(|err| io::Error::other(format!("failed to install app zip {}: {err}", zip_path)).into())
}

#[command]
#[specta::specta]
pub async fn install_app_url(
    app: AppHandle,
    state: State<'_, AppState>,
    app_url: String,
    granted_permissions: SageGrantedPermissions,
) -> Result<InstalledSageApp> {
    let base_path = {
        let state = state.lock().await;
        state.path.clone()
    };

    let root = apps_root(&base_path);

    fs::create_dir_all(&root).map_err(|err| {
        io::Error::other(format!("failed to create apps directory {}: {err}", root.display()))
    })?;

    let preview = preview_app_url_internal(app_url.clone())
        .await
        .map_err(|err| io::Error::other(format!("failed to preview app URL {}: {err}", app_url)))?;

    let granted_permissions =
        normalize_and_validate_granted_permissions(&preview.manifest.permissions, granted_permissions)
            .map_err(|err| {
                io::Error::other(format!("invalid granted permissions for URL app {}: {err}", app_url))
            })?;

    let permission_flags = resolve_capability_flags(&granted_permissions.capabilities, None)
        .map_err(|err| {
            io::Error::other(format!("invalid granted permission policy for URL app {}: {err}", app_url))
        })?;

    let (app_id, install_dir, existing_app) =
        resolve_url_install_target(&root, &preview.manifest_url)
            .map_err(|err| io::Error::other(format!("failed to resolve install dir: {err}")))?;

    let storage = resolve_storage_for_install(&app, &base_path, existing_app.as_ref())
        .await
        .map_err(|err| io::Error::other(format!("failed to resolve storage for install: {err}")))?;

    recreate_install_dir(&install_dir)
        .map_err(|err| io::Error::other(format!("failed to recreate install dir: {err}")))?;

    let snapshot = download_url_snapshot(
        &install_dir,
        &preview.app_url,
        &preview.manifest,
        &preview.manifest_hash,
    )
    .await
    .map_err(|err| io::Error::other(format!("failed to download URL app snapshot: {err}")))?;

    let installed = build_installed_app(
        app_id.clone(),
        &install_dir,
        &preview.manifest,
        granted_permissions,
        permission_flags,
        storage,
        InstalledSageAppSource::Url {
            app_url: preview.app_url.clone(),
            manifest_url: preview.manifest_url.clone(),
        },
        snapshot,
    );

    write_installed_app_metadata(&installed, &install_dir).map_err(|err| {
        io::Error::other(format!("failed to write installed app metadata for {}: {err}", app_id))
    })?;

    Ok(installed)
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
        if let Err(err) = apps_clear_runtime_browsing_data(app.clone(), app_id.clone()).await {
            enqueue_pending_storage_cleanup(&base_path, installed, &err)
                .map_err(|queue_err| {
                    io::Error::other(format!(
                        "failed to enqueue pending storage cleanup after clear failure ({err}): {queue_err}"
                    ))
                })?;
        }
    }

    let install_dir = apps_root(&base_path).join(&app_id);
    if install_dir.exists() {
        fs::remove_dir_all(&install_dir).map_err(|err| {
            io::Error::other(format!(
                "failed to remove installed app {} at {}: {err}",
                app_id,
                install_dir.display()
            ))
        })?;
    }

    Ok(())
}

pub async fn retry_pending_storage_cleanup_on_startup(
    app: &AppHandle,
    base_path: &Path,
) -> AnyResult<()> {
    retry_pending_storage_cleanup(app, base_path).await
}
