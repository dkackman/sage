use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result as AnyResult, anyhow};
use reqwest::{Client, Url};

use crate::{
    lifecycle::limits::{MAX_APP_TOTAL_SIZE_BYTES, MAX_MANIFEST_SIZE_BYTES},
    lifecycle::manifest::validate_package_manifest,
    types::{InstalledSageAppSnapshot, SageAppPackageManifest},
};

fn is_local_dev_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost") || host == "127.0.0.1" || host == "::1"
}

fn app_download_client_for_url(url: &Url) -> AnyResult<Client> {
    let host = url
        .host_str()
        .ok_or_else(|| anyhow!("URL is missing host"))?;

    let mut builder = Client::builder();

    if url.scheme() == "https" && is_local_dev_host(host) {
        builder = builder.danger_accept_invalid_certs(true);
    }

    builder
        .build()
        .context("failed to build app download HTTP client")
}

pub fn derive_manifest_url(app_url: &str) -> AnyResult<String> {
    let mut url = Url::parse(app_url).context("invalid app URL")?;
    if !url.path().ends_with('/') {
        let path = format!("{}/", url.path());
        url.set_path(&path);
    }
    url.set_path(&format!("{}sage-manifest.json", url.path()));
    Ok(url.to_string())
}

pub async fn fetch_url_manifest(
    manifest_url: &str,
) -> AnyResult<(SageAppPackageManifest, String)> {
    let url = Url::parse(manifest_url).context("invalid manifest URL")?;
    let client = app_download_client_for_url(&url)?;

    let response = client.get(url).send().await?;
    let status = response.status();
    if !status.is_success() {
        anyhow::bail!("manifest request failed with status {}", status);
    }

    let bytes = response.bytes().await?;
    if bytes.len() > MAX_MANIFEST_SIZE_BYTES as usize {
        anyhow::bail!("manifest exceeds size limit");
    }

    let manifest_hash = {
        let mut hasher = sha2::Sha256::new();
        use sha2::Digest;
        hasher.update(&bytes);
        hex::encode(hasher.finalize())
    };

    let manifest: SageAppPackageManifest =
        serde_json::from_slice(&bytes).context("failed to parse manifest JSON")?;
    validate_package_manifest(&manifest)?;

    Ok((manifest, manifest_hash))
}

pub async fn download_url_snapshot(
    install_dir: &Path,
    app_url: &str,
    manifest: &SageAppPackageManifest,
    manifest_hash: &str,
) -> AnyResult<InstalledSageAppSnapshot> {
    let snapshot_dir = install_dir.join("active");
    if snapshot_dir.exists() {
        fs::remove_dir_all(&snapshot_dir)?;
    }
    fs::create_dir_all(&snapshot_dir)?;

    let base_url = Url::parse(app_url).context("invalid app URL")?;
    let client = app_download_client_for_url(&base_url)?;

    let mut total_bytes = 0_u64;

    for file in &manifest.files {
        total_bytes = total_bytes.saturating_add(file.size);
        if total_bytes > MAX_APP_TOTAL_SIZE_BYTES {
            anyhow::bail!("app exceeds total size limit");
        }

        let file_url = base_url
            .join(&file.path)
            .with_context(|| format!("failed to join app URL with {}", file.path))?;

        let response = client.get(file_url).send().await?;
        let status = response.status();
        if !status.is_success() {
            anyhow::bail!("file download failed with status {}", status);
        }

        let bytes = response.bytes().await?;
        let actual_size = u64::try_from(bytes.len()).context("downloaded file too large")?;
        if actual_size != file.size {
            anyhow::bail!("downloaded file size mismatch for {}", file.path);
        }

        let mut hasher = sha2::Sha256::new();
        use sha2::Digest;
        hasher.update(&bytes);
        let actual_hash = hex::encode(hasher.finalize());
        if actual_hash != file.sha256 {
            anyhow::bail!("downloaded file hash mismatch for {}", file.path);
        }

        let dst = snapshot_dir.join(&file.path);
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&dst, &bytes)
            .with_context(|| format!("failed to write downloaded file {}", dst.display()))?;
    }

    Ok(InstalledSageAppSnapshot {
        manifest_hash: manifest_hash.to_string(),
        snapshot_dir: snapshot_dir.to_string_lossy().to_string(),
        total_bytes,
        manifest: manifest.clone(),
    })
}

pub fn read_snapshot_file(root: &Path, request_path: &str) -> AnyResult<PathBuf> {
    let normalized = if request_path.is_empty() || request_path == "/" {
        PathBuf::from("../../../../index.html")
    } else {
        PathBuf::from(request_path.trim_start_matches('/'))
    };

    if normalized
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        anyhow::bail!("path traversal denied");
    }

    let file_path = root.join(normalized);
    if !file_path.is_file() {
        anyhow::bail!("file not found: {}", file_path.display());
    }

    Ok(file_path)
}
