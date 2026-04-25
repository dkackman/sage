use std::path::{Path, PathBuf};

use anyhow::{Context, Result as AnyResult, anyhow};
use async_trait::async_trait;
use uuid::Uuid;
use url::Url;

use crate::lifecycle::{derive_manifest_url, download_url_snapshot, fetch_url_manifest, read_retired_app_origins, AppInstallSource};
use crate::lifecycle::registry::read_installed_app_by_id;
use crate::permissions::normalize_and_validate_requested_permissions;
use crate::types::{
    SageAppPackageManifest, SageAppSnapshot, SageAppUrlPreview,
    UserSageApp, UserSageAppSource,
};
use crate::utils::bytes_sha256_hex;

#[derive(Debug, Clone)]
pub struct UrlInstallSource {
    pub app_url: String,
}

#[derive(Debug, Clone)]
pub struct PreparedUrlInstall {
    pub preview: SageAppUrlPreview,
}

#[async_trait]
impl AppInstallSource for UrlInstallSource {
    type Prepared = PreparedUrlInstall;

    async fn prepare(&self) -> AnyResult<Self::Prepared> {
        Ok(PreparedUrlInstall {
            preview: preview_app_url_internal(self.app_url.clone()).await?,
        })
    }

    fn manifest<'a>(&self, prepared: &'a Self::Prepared) -> &'a SageAppPackageManifest {
        &prepared.preview.manifest
    }

    fn source(&self, prepared: &Self::Prepared) -> UserSageAppSource {
        UserSageAppSource::Url {
            app_url: prepared.preview.app_url.clone(),
            manifest_url: prepared.preview.manifest_url.clone(),
        }
    }

    fn resolve_target(
        &self,
        root: &Path,
        _base_path: &Path,
        prepared: &Self::Prepared,
    ) -> AnyResult<(String, PathBuf, Option<UserSageApp>)> {
        resolve_url_install_target(root, &prepared.preview.manifest_url)
    }

    async fn create_snapshot(
        &self,
        app_dir: &Path,
        prepared: &Self::Prepared,
    ) -> AnyResult<SageAppSnapshot> {
        download_url_snapshot(
            app_dir,
            &prepared.preview.app_url,
            &prepared.preview.manifest,
            &prepared.preview.manifest_hash,
        )
            .await
    }

    fn origin_id(
        &self,
        base_path: &Path,
        app_id: &str,
        existing: Option<&UserSageApp>,
    ) -> AnyResult<String> {
        if let Some(existing) = existing {
            return Ok(existing.common.origin_id.clone());
        }

        if should_rotate_url_origin_on_install(base_path, app_id)? {
            Ok(generate_rotated_url_origin_id(app_id))
        } else {
            Ok(default_url_origin_id(app_id))
        }
    }
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

pub fn normalize_app_url(url: &str) -> AnyResult<String> {
    let mut parsed = Url::parse(url).context("invalid app URL")?;

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

pub fn generate_url_app_id(manifest_url: &str) -> String {
    let hash = bytes_sha256_hex(manifest_url.as_bytes());
    format!("url-{}-{}", slugify_host(manifest_url), &hash[..16])
}

pub fn default_url_origin_id(app_id: &str) -> String {
    app_id.to_string()
}

pub fn generate_rotated_url_origin_id(app_id: &str) -> String {
    let suffix = Uuid::new_v4().simple().to_string();
    format!("r{}-{}", &suffix[..12], app_id)
}

pub fn should_rotate_url_origin_on_install(
    base_path: &Path,
    app_id: &str,
) -> AnyResult<bool> {
    let retired = read_retired_app_origins(base_path)?;
    Ok(retired.iter().any(|entry| entry.app_id == app_id))
}

pub fn resolve_url_install_target(
    root: &Path,
    manifest_url: &str,
) -> AnyResult<(String, PathBuf, Option<UserSageApp>)> {
    let app_id = generate_url_app_id(manifest_url);
    let app_dir = root.join(&app_id);

    let existing = if app_dir.exists() {
        Some(read_installed_app_by_id(root.parent().unwrap_or(root), &app_id)?)
    } else {
        None
    };

    Ok((app_id.clone(), app_dir, existing))
}

fn normalize_manifest_permissions(
    mut manifest: SageAppPackageManifest,
) -> AnyResult<SageAppPackageManifest> {
    manifest.permissions = normalize_and_validate_requested_permissions(&manifest.permissions)?;
    Ok(manifest)
}

fn slugify_host(input: &str) -> String {
    if let Ok(url) = Url::parse(input) {
        if let Some(host) = url.host_str() {
            return slugify_name(host);
        }
    }

    slugify_name(input)
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
