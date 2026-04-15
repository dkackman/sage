use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result as AnyResult};

use crate::apps::types::{
    CorruptedInstalledSageApp, InstalledSageApp, ListedSageApp,
};

const INSTALLED_METADATA_FILE: &str = ".sage-installed.json";

pub fn apps_root(base_path: &Path) -> PathBuf {
    base_path.join("apps")
}

pub fn app_install_dir(base_path: &Path, app_id: &str) -> PathBuf {
    apps_root(base_path).join(app_id)
}

pub fn installed_metadata_path(install_dir: &Path) -> PathBuf {
    install_dir.join(INSTALLED_METADATA_FILE)
}

pub fn read_installed_app_from_dir(dir: &Path) -> AnyResult<InstalledSageApp> {
    let path = installed_metadata_path(dir);
    let text =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let app: InstalledSageApp =
        serde_json::from_str(&text).context("failed to parse installed app metadata")?;
    Ok(app)
}

pub fn write_installed_app_metadata(
    app: &InstalledSageApp,
    install_dir: &Path,
) -> AnyResult<()> {
    let path = installed_metadata_path(install_dir);
    let text = serde_json::to_string_pretty(app)
        .map_err(|err| anyhow::anyhow!("failed to serialize installed app metadata: {err}"))?;
    fs::write(path, format!("{text}\n"))?;
    Ok(())
}

pub fn read_installed_app_by_id(
    base_path: &Path,
    app_id: &str,
) -> AnyResult<InstalledSageApp> {
    let install_dir = app_install_dir(base_path, app_id);
    read_installed_app_from_dir(&install_dir)
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
