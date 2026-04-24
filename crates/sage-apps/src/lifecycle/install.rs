use std::{
    fs, io,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result as AnyResult, anyhow};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, State, command};
use uuid::Uuid;

use crate::host::{AppState, Result};
use crate::lifecycle::{
    apps_root, derive_manifest_url, download_url_snapshot, enqueue_pending_storage_cleanup,
    enqueue_retired_app_origin, fetch_url_manifest, list_installed_apps_internal,
    prepare_zip_snapshot, read_manifest, read_retired_app_origins,
    unzip_to_dir, validate_package_structure,
    write_installed_app_metadata,
};
use crate::lifecycle::registry::read_installed_app_by_id;
use crate::permissions::{normalize_and_validate_granted_permissions, normalize_and_validate_requested_permissions, resolve_capability_flags, resolve_effective_granted_capabilities};
use crate::runtime::apps_clear_runtime_browsing_data;
use crate::types::{
    InstalledSageAppStorage, ListedSageApp, SageAppCommon,
    SageAppPackageManifest, SageAppSnapshot, SageAppUrlPreview,
    UserSageApp, UserSageAppSource, SageGrantedPermissions,
};

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

fn default_url_origin_id(app_id: &str) -> String {
    app_id.to_string()
}

fn generate_rotated_url_origin_id(app_id: &str) -> String {
    let suffix = Uuid::new_v4().simple().to_string();
    format!("r{}-{}", &suffix[..12], app_id)
}

fn should_rotate_url_origin_on_install(
    base_path: &Path,
    app_id: &str,
) -> AnyResult<bool> {
    let retired = read_retired_app_origins(base_path)?;
    Ok(retired.iter().any(|entry| entry.app_id == app_id))
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
) -> AnyResult<Option<UserSageApp>> {
    Ok(list_installed_apps_internal(root)?
        .into_iter()
        .find_map(|app| match app {
            ListedSageApp::User(installed) if installed.common.name == app_name => {
                Some(installed)
            }
            _ => None,
        }))
}

fn build_installed_app(
    app_id: String,
    origin_id: String,
    app_dir: &Path,
    manifest: &SageAppPackageManifest,
    granted_permissions: SageGrantedPermissions,
    permission_flags: crate::types::SageAppCapabilityFlags,
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

fn recreate_app_dir(app_dir: &Path) -> AnyResult<()> {
    if app_dir.exists() {
        fs::remove_dir_all(app_dir).with_context(|| {
            format!("failed to remove existing app dir {}", app_dir.display())
        })?;
    }

    fs::create_dir_all(app_dir)
        .with_context(|| format!("failed to create app dir {}", app_dir.display()))?;

    Ok(())
}

fn resolve_zip_install_target(
    root: &Path,
    app_name: &str,
) -> AnyResult<(String, std::path::PathBuf, Option<UserSageApp>)> {
    if let Some(existing) = find_existing_installed_app_by_name(root, app_name)? {
        let app_dir = Path::new(&existing.common.app_dir).to_path_buf();
        return Ok((existing.common.id.clone(), app_dir, Some(existing)));
    }

    let app_id = generate_zip_app_id(app_name);
    Ok((app_id.clone(), root.join(&app_id), None))
}

fn resolve_url_install_target(
    root: &Path,
    manifest_url: &str,
) -> AnyResult<(String, std::path::PathBuf, Option<UserSageApp>)> {
    let app_id = generate_url_app_id(manifest_url);
    let app_dir = root.join(&app_id);

    let existing = if app_dir.exists() {
        Some(read_installed_app_by_id(root.parent().unwrap_or(root), &app_id)?)
    } else {
        None
    };

    Ok((app_id.clone(), app_dir, existing))
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
    Ok(InstalledSageAppStorage::Unmanaged)
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
        let package_root = crate::lifecycle::detect_package_root(&unpack_dir)?;
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
) -> Result<UserSageApp> {
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

    let result: AnyResult<UserSageApp> = async {
        unzip_to_dir(Path::new(&zip_path), &unpack_dir)?;
        let package_root = crate::lifecycle::detect_package_root(&unpack_dir)?;
        validate_package_structure(&package_root)?;

        let manifest = normalize_manifest_permissions(read_manifest(&package_root)?)?;

        let granted_permissions =
            normalize_and_validate_granted_permissions(&manifest.permissions, granted_permissions)?;

        let effective_capabilities = resolve_effective_granted_capabilities(
            &manifest.permissions,
            &granted_permissions.capabilities,
        )?;
        let permission_flags = resolve_capability_flags(&effective_capabilities, None)?;

        let (app_id, app_dir, existing_app) = resolve_zip_install_target(&root, &manifest.name)?;
        let storage = resolve_storage_for_install(&app, &base_path, existing_app.as_ref()).await?;

        recreate_app_dir(&app_dir)?;
        let snapshot = prepare_zip_snapshot(&package_root, &app_dir, &manifest)?;

        let installed = build_installed_app(
            app_id.clone(),
            app_id,
            &app_dir,
            &manifest,
            granted_permissions,
            permission_flags,
            storage,
            UserSageAppSource::Zip,
            snapshot,
        );

        write_installed_app_metadata(&installed, &app_dir)?;
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
) -> Result<UserSageApp> {
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

    let effective_capabilities = resolve_effective_granted_capabilities(
        &preview.manifest.permissions,
        &granted_permissions.capabilities,
    )
        .map_err(|err| {
            io::Error::other(format!("invalid granted permission policy for URL app {}: {err}", app_url))
        })?;

    let permission_flags = resolve_capability_flags(&effective_capabilities, None)
        .map_err(|err| {
            io::Error::other(format!("invalid granted permission policy for URL app {}: {err}", app_url))
        })?;

    let (app_id, app_dir, existing_app) =
        resolve_url_install_target(&root, &preview.manifest_url)
            .map_err(|err| io::Error::other(format!("failed to resolve app dir: {err}")))?;

    let storage = resolve_storage_for_install(&app, &base_path, existing_app.as_ref())
        .await
        .map_err(|err| io::Error::other(format!("failed to resolve storage for install: {err}")))?;

    recreate_app_dir(&app_dir)
        .map_err(|err| io::Error::other(format!("failed to recreate app dir: {err}")))?;

    let snapshot = download_url_snapshot(
        &app_dir,
        &preview.app_url,
        &preview.manifest,
        &preview.manifest_hash,
    )
        .await
        .map_err(|err| io::Error::other(format!("failed to download URL app snapshot: {err}")))?;

    let origin_id = match existing_app.as_ref() {
        Some(existing) => existing.common.origin_id.clone(),
        None => {
            if should_rotate_url_origin_on_install(&base_path, &app_id)
                .map_err(|err| io::Error::other(format!("failed to inspect retired origins: {err}")))? {
                generate_rotated_url_origin_id(&app_id)
            } else {
                default_url_origin_id(&app_id)
            }
        }
    };

    let installed = build_installed_app(
        app_id.clone(),
        origin_id,
        &app_dir,
        &preview.manifest,
        granted_permissions,
        permission_flags,
        storage,
        UserSageAppSource::Url {
            app_url: preview.app_url.clone(),
            manifest_url: preview.manifest_url.clone(),
        },
        snapshot,
    );

    write_installed_app_metadata(&installed, &app_dir).map_err(|err| {
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
                enqueue_pending_storage_cleanup(&base_path, installed, &err).map_err(|queue_err| {
                    io::Error::other(format!(
                        "failed to enqueue pending storage cleanup after clear failure ({err}): {queue_err}"
                    ))
                })?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;
    use crate::bridge::capabilities::UserBridgeCapability;
    use crate::lifecycle::{
        write_retired_app_origins,
    };
    use crate::permissions::normalize_and_validate_granted_network_whitelist;
    use crate::types::{InstalledSageAppStorage, RetiredAppOriginEntry, SageAppCapabilityFlags, SageAppManifestFile, SageAppPackageManifest, SageGrantedNetworkPermissions, SageGrantedPermissions, SageNetworkPermissionTarget, SageRequestedCapabilities, SageRequestedNetworkPermissions, SageRequestedNetworkWhitelist, SageRequestedPermissions};

    fn sample_manifest() -> SageAppPackageManifest {
        SageAppPackageManifest {
            name: "Test App".into(),
            version: "1.0.0".into(),
            permissions: SageRequestedPermissions {
                network: SageRequestedNetworkPermissions {
                    whitelist: SageRequestedNetworkWhitelist {
                        required: vec![SageNetworkPermissionTarget {
                            scheme: "https".into(),
                            host: "api.example.com".into(),
                        }],
                        optional: vec![SageNetworkPermissionTarget {
                            scheme: "wss".into(),
                            host: "ws.example.com".into(),
                        }],
                    },
                },
                capabilities: SageRequestedCapabilities {
                    required: vec![UserBridgeCapability::PersistentStorage],
                    optional: vec![UserBridgeCapability::WalletSendXch],
                },
            },
            files: vec![SageAppManifestFile {
                path: "index.html".into(),
                sha256: "a".repeat(64),
                size: 123,
            }],
            entry: Some("index.html".into()),
            icon: Some("icon.png".into()),
            author: None,
            donation: None,
        }
    }

    fn sample_app(app_id: &str, origin_id: &str) -> UserSageApp {
        let dir = tempdir().unwrap();
        let app_dir = dir.path().join(app_id);
        fs::create_dir_all(&app_dir).unwrap();

        UserSageApp {
            common: SageAppCommon {
                id: app_id.into(),
                origin_id: origin_id.into(),
                name: "Test App".into(),
                version: "1.0.0".into(),
                app_dir: app_dir.to_string_lossy().to_string(),
                entry_file: "index.html".into(),
                icon_file: "icon.png".into(),
                requested_permissions: sample_manifest().permissions.clone(),
                granted_permissions: SageGrantedPermissions {
                    capabilities: vec![UserBridgeCapability::PersistentStorage],
                    network: SageGrantedNetworkPermissions {
                        whitelist: vec![SageNetworkPermissionTarget {
                            scheme: "https".into(),
                            host: "api.example.com".into(),
                        }],
                    },
                },
                capability_flags: SageAppCapabilityFlags::default(),
                storage: InstalledSageAppStorage::Unmanaged,
                active_snapshot: SageAppSnapshot {
                    manifest_hash: "hash".into(),
                    snapshot_dir: app_dir.to_string_lossy().to_string(),
                    total_bytes: 123,
                    manifest: sample_manifest(),
                },
            },
            source: UserSageAppSource::Url {
                app_url: "https://example.com/app/".into(),
                manifest_url: "https://example.com/app/sage-manifest.json".into(),
            },
            pending_update: None,
        }
    }

    #[test]
    fn normalize_app_url_keeps_https_and_adds_trailing_slash() {
        let out = normalize_app_url("https://example.com/app").unwrap();
        assert_eq!(out, "https://example.com/app/");
    }

    #[test]
    fn normalize_app_url_strips_query_and_fragment() {
        let out = normalize_app_url("https://example.com/app?x=1#frag").unwrap();
        assert_eq!(out, "https://example.com/app/");
    }

    #[test]
    fn normalize_app_url_allows_localhost_http() {
        let out = normalize_app_url("http://localhost:4173").unwrap();
        assert_eq!(out, "http://localhost:4173/");
    }

    #[test]
    fn normalize_app_url_rejects_non_local_http() {
        let err = normalize_app_url("http://example.com/app").unwrap_err().to_string();
        assert!(err.contains("requires HTTPS"));
    }

    #[test]
    fn generate_url_app_id_is_stable_for_same_manifest_url() {
        let a = generate_url_app_id("https://example.com/app/sage-manifest.json");
        let b = generate_url_app_id("https://example.com/app/sage-manifest.json");
        assert_eq!(a, b);
        assert!(a.starts_with("url-"));
    }

    #[test]
    fn default_url_origin_id_is_same_as_app_id() {
        assert_eq!(default_url_origin_id("url-abc123"), "url-abc123");
    }

    #[test]
    fn rotated_url_origin_id_differs_from_app_id() {
        let origin = generate_rotated_url_origin_id("url-abc123");
        assert_ne!(origin, "url-abc123");
        assert!(origin.ends_with("url-abc123"));
        assert!(origin.starts_with('r'));
    }

    #[test]
    fn granted_network_whitelist_always_includes_required_and_selected_optional() {
        let requested = SageRequestedNetworkPermissions {
            whitelist: SageRequestedNetworkWhitelist {
                required: vec![SageNetworkPermissionTarget {
                    scheme: "https".into(),
                    host: "api.example.com".into(),
                }],
                optional: vec![SageNetworkPermissionTarget {
                    scheme: "wss".into(),
                    host: "ws.example.com".into(),
                }],
            },
        };

        let granted = vec![SageNetworkPermissionTarget {
            scheme: "wss".into(),
            host: "ws.example.com".into(),
        }];

        let result =
            normalize_and_validate_granted_network_whitelist(&requested, &granted).unwrap();

        assert_eq!(
            result,
            vec![
                SageNetworkPermissionTarget {
                    scheme: "https".into(),
                    host: "api.example.com".into(),
                },
                SageNetworkPermissionTarget {
                    scheme: "wss".into(),
                    host: "ws.example.com".into(),
                },
            ]
        );
    }

    #[test]
    fn granted_network_whitelist_rejects_unrequested_entry() {
        let requested = SageRequestedNetworkPermissions {
            whitelist: SageRequestedNetworkWhitelist {
                required: vec![],
                optional: vec![],
            },
        };

        let granted = vec![SageNetworkPermissionTarget {
            scheme: "https".into(),
            host: "evil.example.com".into(),
        }];

        let err =
            normalize_and_validate_granted_network_whitelist(&requested, &granted).unwrap_err();

        assert!(err
            .to_string()
            .contains("granted network whitelist entry not requested"));
    }

    #[test]
    fn should_rotate_url_origin_on_install_is_false_without_retired_entry() {
        let dir = tempdir().unwrap();
        assert!(!should_rotate_url_origin_on_install(dir.path(), "url-abc123").unwrap());
    }

    #[test]
    fn should_rotate_url_origin_on_install_is_true_with_retired_entry() {
        let dir = tempdir().unwrap();

        write_retired_app_origins(
            dir.path(),
            &[RetiredAppOriginEntry {
                id: "retired-1".into(),
                app_id: "url-abc123".into(),
                app_name: "Test App".into(),
                origin_id: "url-abc123".into(),
                created_at_ms: 1,
                storage_may_contain_secrets: true,
                cleanup_pending: true,
            }],
        )
            .unwrap();

        assert!(should_rotate_url_origin_on_install(dir.path(), "url-abc123").unwrap());
    }

    #[test]
    fn build_installed_app_sets_id_and_origin_id_independently() {
        let dir = tempdir().unwrap();
        let app_dir = dir.path().join("url-abc123");
        fs::create_dir_all(&app_dir).unwrap();

        let manifest = sample_manifest();
        let app = build_installed_app(
            "url-abc123".into(),
            "r123-url-abc123".into(),
            &app_dir,
            &manifest,
            SageGrantedPermissions {
                capabilities: vec![UserBridgeCapability::PersistentStorage],
                network: SageGrantedNetworkPermissions { whitelist: vec![] },
            },
            SageAppCapabilityFlags::default(),
            InstalledSageAppStorage::Unmanaged,
            UserSageAppSource::Url {
                app_url: "https://example.com/app/".into(),
                manifest_url: "https://example.com/app/sage-manifest.json".into(),
            },
            SageAppSnapshot {
                manifest_hash: "hash".into(),
                snapshot_dir: app_dir.to_string_lossy().to_string(),
                total_bytes: 1,
                manifest: manifest.clone(),
            },
        );

        assert_eq!(app.common.id, "url-abc123");
        assert_eq!(app.common.origin_id, "r123-url-abc123");
    }

    #[test]
    fn sample_app_helper_is_valid_shape() {
        let app = sample_app("url-abc123", "origin-1");
        assert_eq!(app.common.id, "url-abc123");
        assert_eq!(app.common.origin_id, "origin-1");
    }
}
