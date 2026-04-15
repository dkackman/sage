use std::{
    fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Context, Result as AnyResult};
use sha2::{Digest, Sha256};
use tauri::{command, State};
use zip::ZipArchive;

use crate::{
    app_state::AppState,
    apps::{
        permissions::{
            validate_granted_permissions_against_requested,
        },
        types::{
            CorruptedInstalledSageApp, InstalledSageApp, ListedSageApp, SageAppPackageManifest, SageGrantedPermissions,
        },
    },
    error::Result,
};
use crate::apps::manifest::validate_package_manifest;
use crate::apps::snapshot::{derive_manifest_url, download_url_snapshot, fetch_url_manifest};
use crate::apps::types::{InstalledSageAppSource, SageAppUrlPreview};

const INSTALLED_METADATA_FILE: &str = ".sage-installed.json";

pub fn apps_root(base_path: &Path) -> PathBuf {
    base_path.join("apps")
}

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

fn generate_app_id(name: &str) -> String {
    format!("{}-{}", slugify_name(name), current_millis())
}

fn should_ignore_root_entry(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
        return false;
    };

    name == "__MACOSX" || name == ".DS_Store"
}

fn copy_dir_all(src: &Path, dst: &Path) -> AnyResult<()> {
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let from = entry.path();
        let to = dst.join(entry.file_name());

        if file_type.is_dir() {
            copy_dir_all(&from, &to)?;
        } else if file_type.is_file() {
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&from, &to)?;
        }
    }

    Ok(())
}

fn prepare_zip_snapshot(
    package_root: &Path,
    install_dir: &Path,
    manifest: &SageAppPackageManifest,
) -> AnyResult<crate::apps::types::InstalledSageAppSnapshot> {
    let manifest_json = serde_json::to_string_pretty(manifest)
        .map_err(|err| anyhow!("failed to serialize manifest: {err}"))?;
    let manifest_hash = hash_string(&manifest_json);

    let snapshot_dir = install_dir.join("snapshots").join(&manifest_hash);
    if snapshot_dir.exists() {
        fs::remove_dir_all(&snapshot_dir)?;
    }
    fs::create_dir_all(&snapshot_dir)?;

    let dist_dir = package_root.join("dist");
    if !dist_dir.is_dir() {
        return Err(anyhow!("package is missing dist directory"));
    }

    copy_dir_all(&dist_dir, &snapshot_dir)?;

    let icon_src = package_root.join("icon.png");
    if icon_src.is_file() {
        fs::copy(&icon_src, snapshot_dir.join("icon.png"))?;
    }

    fs::write(
        snapshot_dir.join("sage-manifest.json"),
        format!("{manifest_json}\n"),
    )?;

    let total_bytes = manifest.files.iter().map(|f| f.size).sum();

    Ok(crate::apps::types::InstalledSageAppSnapshot {
        manifest_hash,
        snapshot_dir: snapshot_dir.to_string_lossy().to_string(),
        total_bytes,
        manifest: manifest.clone(),
    })
}

fn unzip_to_dir(zip_path: &Path, out_dir: &Path) -> AnyResult<()> {
    let file = fs::File::open(zip_path)
        .with_context(|| format!("failed to open zip file {}", zip_path.display()))?;
    let mut archive = ZipArchive::new(file).context("failed to read zip archive")?;

    fs::create_dir_all(out_dir)?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let enclosed = entry
            .enclosed_name()
            .ok_or_else(|| anyhow!("zip contains invalid path"))?
            .to_path_buf();

        let out_path = out_dir.join(enclosed);

        if entry.name().ends_with('/') {
            fs::create_dir_all(&out_path)?;
            continue;
        }

        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut outfile = fs::File::create(&out_path)?;
        io::copy(&mut entry, &mut outfile)?;
    }

    Ok(())
}

fn detect_package_root(unpack_dir: &Path) -> AnyResult<PathBuf> {
    if unpack_dir.join("manifest.json").exists() {
        return Ok(unpack_dir.to_path_buf());
    }

    let mut candidates = Vec::new();

    for entry in fs::read_dir(unpack_dir)? {
        let entry = entry?;
        let path = entry.path();

        if should_ignore_root_entry(&path) {
            continue;
        }

        candidates.push(path);
    }

    if candidates.len() == 1 && candidates[0].is_dir() {
        let candidate = &candidates[0];
        if candidate.join("manifest.json").exists() {
            return Ok(candidate.clone());
        }
    }

    Err(anyhow!(
        "zip must contain manifest.json at root or inside a single top-level folder"
    ))
}

fn read_manifest(package_root: &Path) -> AnyResult<SageAppPackageManifest> {
    let manifest_path = package_root.join("manifest.json");
    let text = fs::read_to_string(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;

    let manifest: SageAppPackageManifest =
        serde_json::from_str(&text).context("failed to parse manifest.json")?;

    validate_manifest(&manifest)?;
    Ok(manifest)
}

fn validate_manifest(manifest: &SageAppPackageManifest) -> AnyResult<()> {
    validate_package_manifest(manifest).map(|_| ())
}

fn validate_package_structure(package_root: &Path) -> AnyResult<()> {
    let icon = package_root.join("icon.png");
    let entry = package_root.join("dist").join("index.html");

    if !icon.is_file() {
        return Err(anyhow!("package is missing icon.png"));
    }

    if !entry.is_file() {
        return Err(anyhow!("package is missing dist/index.html"));
    }

    Ok(())
}

fn installed_metadata_path(install_dir: &Path) -> PathBuf {
    install_dir.join(INSTALLED_METADATA_FILE)
}

fn read_installed_app_from_dir(dir: &Path) -> AnyResult<InstalledSageApp> {
    let path = installed_metadata_path(dir);
    let text =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let app: InstalledSageApp =
        serde_json::from_str(&text).context("failed to parse installed app metadata")?;
    Ok(app)
}

fn write_installed_app_metadata(app: &InstalledSageApp, install_dir: &Path) -> AnyResult<()> {
    let path = installed_metadata_path(install_dir);
    let text = serde_json::to_string_pretty(app)
        .map_err(|err| anyhow!("failed to serialize installed app metadata: {err}"))?;
    fs::write(path, format!("{text}\n"))?;
    Ok(())
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

fn merge_granted_permissions_for_update(
    old_granted: &SageGrantedPermissions,
    new_requested: &crate::apps::types::SageRequestedPermissions,
    user_selected: &SageGrantedPermissions,
) -> SageGrantedPermissions {
    let requested_network: std::collections::BTreeSet<(String, String)> = new_requested
        .network
        .iter()
        .map(|entry| (entry.scheme.to_ascii_lowercase(), entry.host.to_ascii_lowercase()))
        .collect();

    let old_network: Vec<crate::apps::types::SageGrantedNetworkPermissionEntry> = old_granted
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

pub fn app_install_dir(base_path: &Path, app_id: &str) -> PathBuf {
    apps_root(base_path).join(app_id)
}

pub fn read_installed_app_by_id(base_path: &Path, app_id: &str) -> AnyResult<InstalledSageApp> {
    let install_dir = app_install_dir(base_path, app_id);
    read_installed_app_from_dir(&install_dir)
}

fn normalize_app_url(url: &str) -> AnyResult<String> {
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

    let path = parsed.path();
    if !path.ends_with('/') {
        parsed.set_path(&format!("{path}/"));
    }

    Ok(parsed.to_string())
}

fn hash_string(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
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
        InstalledSageAppSource::Url { app_url, manifest_url } => (app_url.clone(), manifest_url.clone()),
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
        InstalledSageAppSource::Url { app_url, manifest_url } => (app_url.clone(), manifest_url.clone()),
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

    app.pending_update = Some(crate::apps::types::InstalledSageAppPendingUpdate {
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

#[command]
#[specta::specta]
pub async fn preview_app_zip(zip_path: String) -> Result<SageAppPackageManifest> {
    let unpack_dir = std::env::temp_dir().join(format!(".sage-preview-{}", current_millis()));

    let result = (|| -> AnyResult<SageAppPackageManifest> {
        unzip_to_dir(Path::new(&zip_path), &unpack_dir)?;
        let package_root = detect_package_root(&unpack_dir)?;
        let manifest = read_manifest(&package_root)?;
        validate_package_structure(&package_root)?;
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
    let app_url = normalize_app_url(&app_url).map_err(|err| {
        io::Error::other(format!("failed to normalize app URL {}: {err}", app_url))
    })?;

    let manifest_url = derive_manifest_url(&app_url).map_err(|err| {
        io::Error::other(format!("failed to derive manifest URL for {}: {err}", app_url))
    })?;

    let (manifest, manifest_hash) = fetch_url_manifest(&manifest_url)
        .await
        .map_err(|err| io::Error::other(format!("failed to preview app URL {}: {err}", app_url)))?;

    Ok(SageAppUrlPreview {
        app_url,
        manifest_url,
        manifest_hash,
        manifest,
    })
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
        io::Error::other(format!(
            "failed to create apps directory {}: {err}",
            root.display()
        ))
    })?;

    list_installed_apps_internal(&root).map_err(|err| {
        io::Error::other(format!("failed to list installed apps: {err}")).into()
    })
}

#[command]
#[specta::specta]
pub async fn install_app_zip(
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
        io::Error::other(format!(
            "failed to create apps directory {}: {err}",
            root.display()
        ))
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

    let result = (|| -> AnyResult<InstalledSageApp> {
        unzip_to_dir(Path::new(&zip_path), &unpack_dir)?;
        let package_root = detect_package_root(&unpack_dir)?;
        validate_package_structure(&package_root)?;
        let manifest = read_manifest(&package_root)?;
        validate_granted_permissions_against_requested(&manifest.permissions, &granted_permissions)?;

        let existing = list_installed_apps_internal(&root)?
            .into_iter()
            .find_map(|app| match app {
                ListedSageApp::Installed(installed) if installed.name == manifest.name => {
                    Some(installed)
                }
                _ => None,
            });

        let app_id = existing
            .map(|app| app.id)
            .unwrap_or_else(|| generate_app_id(&manifest.name));

        let install_dir = root.join(&app_id);
        if install_dir.exists() {
            fs::remove_dir_all(&install_dir).with_context(|| {
                format!(
                    "failed to remove existing install dir {}",
                    install_dir.display()
                )
            })?;
        }

        copy_dir_all(&package_root, &install_dir)?;

        let snapshot = prepare_zip_snapshot(&package_root, &install_dir, &manifest)?;

        let installed = InstalledSageApp {
            id: app_id,
            name: manifest.name.clone(),
            version: manifest.version.clone(),
            install_dir: install_dir.to_string_lossy().to_string(),
            entry_file: Path::new(&snapshot.snapshot_dir)
                .join("index.html")
                .to_string_lossy()
                .to_string(),
            icon_file: Path::new(&snapshot.snapshot_dir)
                .join("icon.png")
                .to_string_lossy()
                .to_string(),
            requested_permissions: manifest.permissions.clone(),
            granted_permissions,
            source: InstalledSageAppSource::Zip,
            active_snapshot: snapshot,
            pending_update: None,
        };

        write_installed_app_metadata(&installed, &install_dir)?;
        Ok(installed)
    })();

    if unpack_dir.exists() {
        let _ = fs::remove_dir_all(&unpack_dir);
    }

    result.map_err(|err| {
        io::Error::other(format!("failed to install app zip {}: {err}", zip_path)).into()
    })
}

#[tauri::command]
#[specta::specta]
pub async fn install_app_url(
    state: tauri::State<'_, crate::app_state::AppState>,
    app_url: String,
    granted_permissions: SageGrantedPermissions,
) -> crate::error::Result<InstalledSageApp> {
    let base_path = {
        let state = state.lock().await;
        state.path.clone()
    };

    let root = super::install::apps_root(&base_path);

    fs::create_dir_all(&root).map_err(|err| {
        io::Error::other(format!(
            "failed to create apps directory {}: {err}",
            root.display()
        ))
    })?;

    let preview = preview_app_url(app_url.clone()).await?;

    super::install::validate_granted_permissions_against_requested(
        &preview.manifest.permissions,
        &granted_permissions,
    )
        .map_err(|err| {
            io::Error::other(format!(
                "invalid granted permissions for URL app {}: {err}",
                app_url
            ))
        })?;

    let existing = super::install::list_installed_apps_internal(&root)
        .map_err(|err| io::Error::other(format!("failed to list installed apps: {err}")))?
        .into_iter()
        .find_map(|app| match app {
            ListedSageApp::Installed(installed) if installed.name == preview.manifest.name => {
                Some(installed)
            }
            _ => None,
        });

    let app_id = existing
        .map(|app| app.id)
        .unwrap_or_else(|| super::install::generate_app_id(&preview.manifest.name));

    let install_dir = root.join(&app_id);
    fs::create_dir_all(&install_dir).map_err(|err| {
        io::Error::other(format!(
            "failed to create install dir {}: {err}",
            install_dir.display()
        ))
    })?;

    let snapshot = download_url_snapshot(
        &install_dir,
        &preview.app_url,
        &preview.manifest,
        &preview.manifest_hash,
    )
        .await
        .map_err(|err| io::Error::other(format!("failed to download URL app snapshot: {err}")))?;

    let installed = InstalledSageApp {
        id: app_id.clone(),
        name: preview.manifest.name.clone(),
        version: preview.manifest.version.clone(),
        install_dir: install_dir.to_string_lossy().to_string(),
        entry_file: Path::new(&snapshot.snapshot_dir)
            .join("index.html")
            .to_string_lossy()
            .to_string(),
        icon_file: Path::new(&snapshot.snapshot_dir)
            .join("icon.png")
            .to_string_lossy()
            .to_string(),
        requested_permissions: preview.manifest.permissions.clone(),
        granted_permissions,
        source: InstalledSageAppSource::Url {
            app_url: preview.app_url.clone(),
            manifest_url: preview.manifest_url.clone(),
        },
        active_snapshot: snapshot,
        pending_update: None,
    };

    write_installed_app_metadata(&installed, &install_dir).map_err(|err| {
        io::Error::other(format!(
            "failed to write installed app metadata for {}: {err}",
            app_id
        ))
    })?;

    Ok(installed)
}

#[command]
#[specta::specta]
pub async fn uninstall_app(state: State<'_, AppState>, app_id: String) -> Result<()> {
    let base_path = {
        let state = state.lock().await;
        state.path.clone()
    };

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
