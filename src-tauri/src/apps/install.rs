use std::{
    fs, io,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Context, Result as AnyResult};
use sha2::{Digest, Sha256};
use tauri::{command, State};

use crate::{
    app_state::AppState,
    apps::{
        package::{
            detect_package_root, prepare_zip_snapshot, read_manifest, unzip_to_dir,
            validate_package_structure,
        },
        permissions::{
            normalize_and_validate_requested_permissions,
            resolve_granted_permission_flags, validate_granted_permissions,
        },
        registry::{
            apps_root, list_installed_apps_internal, write_installed_app_metadata,
        },
        snapshot::{derive_manifest_url, download_url_snapshot, fetch_url_manifest},
        types::{
            InstalledSageApp, InstalledSageAppSource, ListedSageApp,
            SageAppPackageManifest, SageAppUrlPreview,
        },
    },
    error::Result,
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

fn generate_app_id(name: &str) -> String {
    format!("{}-{}", slugify_name(name), current_millis())
}

pub fn normalize_app_url(url: &str) -> AnyResult<String> {
    let mut parsed = reqwest::Url::parse(url).context("invalid app URL")?;

    let scheme = parsed.scheme();
    let host = parsed
        .host_str()
        .ok_or_else(|| anyhow!("app URL is missing host"))?
        .to_ascii_lowercase();

    let is_local_dev_host =
        host == "localhost" || host == "127.0.0.1" || host == "::1";

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

pub fn hash_string(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

fn normalize_manifest_permissions(
    mut manifest: SageAppPackageManifest,
) -> AnyResult<SageAppPackageManifest> {
    manifest.permissions =
        normalize_and_validate_requested_permissions(&manifest.permissions)?;
    Ok(manifest)
}

pub async fn preview_app_url_internal(
    app_url: String,
) -> AnyResult<SageAppUrlPreview> {
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
    let unpack_dir =
        std::env::temp_dir().join(format!(".sage-preview-{}", current_millis()));

    let result = (|| -> AnyResult<SageAppPackageManifest> {
        unzip_to_dir(Path::new(&zip_path), &unpack_dir)?;
        let package_root = detect_package_root(&unpack_dir)?;
        let manifest = read_manifest(&package_root)?;
        validate_package_structure(&package_root)?;
        normalize_manifest_permissions(manifest)
    })();

    let _ = fs::remove_dir_all(&unpack_dir);

    result.map_err(|err| {
        io::Error::other(format!(
            "failed to preview app zip {}: {err}",
            zip_path
        ))
            .into()
    })
}

#[command]
#[specta::specta]
pub async fn preview_app_url(app_url: String) -> Result<SageAppUrlPreview> {
    preview_app_url_internal(app_url).await.map_err(|err| {
        io::Error::other(format!("failed to preview app URL: {err}")).into()
    })
}

#[command]
#[specta::specta]
pub async fn list_installed_apps(
    state: State<'_, AppState>,
) -> Result<Vec<ListedSageApp>> {
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
    granted_permissions: Vec<String>,
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

        let manifest = normalize_manifest_permissions(read_manifest(&package_root)?)?;

        validate_granted_permissions(&manifest.permissions, &granted_permissions)?;
        let permission_flags =
            resolve_granted_permission_flags(&granted_permissions, None)?;

        let existing = list_installed_apps_internal(&root)?
            .into_iter()
            .find_map(|app| match app {
                ListedSageApp::Installed(installed)
                if installed.name == manifest.name =>
                    {
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

        fs::create_dir_all(&install_dir)?;
        let snapshot =
            prepare_zip_snapshot(&package_root, &install_dir, &manifest)?;

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
            permission_flags,
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
        io::Error::other(format!(
            "failed to install app zip {}: {err}",
            zip_path
        ))
            .into()
    })
}

#[command]
#[specta::specta]
pub async fn install_app_url(
    state: State<'_, AppState>,
    app_url: String,
    granted_permissions: Vec<String>,
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

    let preview = preview_app_url_internal(app_url.clone())
        .await
        .map_err(|err| {
            io::Error::other(format!(
                "failed to preview app URL {}: {err}",
                app_url
            ))
        })?;

    validate_granted_permissions(
        &preview.manifest.permissions,
        &granted_permissions,
    )
        .map_err(|err| {
            io::Error::other(format!(
                "invalid granted permissions for URL app {}: {err}",
                app_url
            ))
        })?;

    let permission_flags =
        resolve_granted_permission_flags(&granted_permissions, None)
            .map_err(|err| {
                io::Error::other(format!(
                    "invalid granted permission policy for URL app {}: {err}",
                    app_url
                ))
            })?;

    let existing = list_installed_apps_internal(&root)
        .map_err(|err| {
            io::Error::other(format!("failed to list installed apps: {err}"))
        })?
        .into_iter()
        .find_map(|app| match app {
            ListedSageApp::Installed(installed)
            if installed.name == preview.manifest.name =>
                {
                    Some(installed)
                }
            _ => None,
        });

    let app_id = existing
        .map(|app| app.id)
        .unwrap_or_else(|| generate_app_id(&preview.manifest.name));

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
        .map_err(|err| {
            io::Error::other(format!(
                "failed to download URL app snapshot: {err}"
            ))
        })?;

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
        permission_flags,
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
pub async fn uninstall_app(
    state: State<'_, AppState>,
    app_id: String,
) -> Result<()> {
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
