use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result as AnyResult;
use async_trait::async_trait;
use uuid::Uuid;

use crate::lifecycle::{detect_package_root, list_installed_apps_internal, prepare_zip_snapshot, read_manifest, unzip_to_dir, validate_package_structure, AppInstallSource};
use crate::permissions::normalize_and_validate_requested_permissions;
use crate::types::{
    ListedSageApp, SageAppPackageManifest, SageAppSnapshot,
    UserSageApp, UserSageAppSource,
};

#[derive(Debug, Clone)]
pub struct ZipInstallSource {
    pub zip_path: String,
    pub unpack_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct PreparedZipInstall {
    pub package_root: PathBuf,
    pub manifest: SageAppPackageManifest,
}

impl ZipInstallSource {
    pub fn new(root: &Path, zip_path: String) -> Self {
        Self {
            zip_path,
            unpack_dir: root.join(format!(".tmp-{}", Uuid::new_v4())),
        }
    }

    pub fn cleanup(&self) {
        if self.unpack_dir.exists() {
            let _ = fs::remove_dir_all(&self.unpack_dir);
        }
    }
}

#[async_trait]
impl AppInstallSource for ZipInstallSource {
    type Prepared = PreparedZipInstall;

    async fn prepare(&self) -> AnyResult<Self::Prepared> {
        if self.unpack_dir.exists() {
            fs::remove_dir_all(&self.unpack_dir)?;
        }

        unzip_to_dir(Path::new(&self.zip_path), &self.unpack_dir)?;

        let package_root = detect_package_root(&self.unpack_dir)?;
        validate_package_structure(&package_root)?;

        let manifest = normalize_manifest_permissions(read_manifest(&package_root)?)?;

        Ok(PreparedZipInstall {
            package_root,
            manifest,
        })
    }

    fn manifest<'a>(&self, prepared: &'a Self::Prepared) -> &'a SageAppPackageManifest {
        &prepared.manifest
    }

    fn source(&self, _prepared: &Self::Prepared) -> UserSageAppSource {
        UserSageAppSource::Zip
    }

    fn resolve_target(
        &self,
        root: &Path,
        _base_path: &Path,
        prepared: &Self::Prepared,
    ) -> AnyResult<(String, PathBuf, Option<UserSageApp>)> {
        resolve_zip_install_target(root, &prepared.manifest.name)
    }

    async fn create_snapshot(
        &self,
        app_dir: &Path,
        prepared: &Self::Prepared,
    ) -> AnyResult<SageAppSnapshot> {
        prepare_zip_snapshot(&prepared.package_root, app_dir, &prepared.manifest)
    }
}

pub fn generate_zip_app_id(name: &str) -> String {
    format!("{}-{}", slugify_name(name), Uuid::new_v4())
}

pub fn resolve_zip_install_target(
    root: &Path,
    app_name: &str,
) -> AnyResult<(String, PathBuf, Option<UserSageApp>)> {
    if let Some(existing) = find_existing_installed_app_by_name(root, app_name)? {
        let app_dir = Path::new(&existing.common.app_dir).to_path_buf();
        return Ok((existing.common.id.clone(), app_dir, Some(existing)));
    }

    let app_id = generate_zip_app_id(app_name);
    Ok((app_id.clone(), root.join(&app_id), None))
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

fn normalize_manifest_permissions(
    mut manifest: SageAppPackageManifest,
) -> AnyResult<SageAppPackageManifest> {
    manifest.permissions = normalize_and_validate_requested_permissions(&manifest.permissions)?;
    Ok(manifest)
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
