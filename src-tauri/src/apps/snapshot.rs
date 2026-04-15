use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context, Result as AnyResult};
use reqwest::Url;
use sha2::{Digest, Sha256};

use crate::apps::{
    limits::{MAX_APP_TOTAL_SIZE_BYTES, MAX_MANIFEST_SIZE_BYTES},
    manifest::validate_package_manifest,
    types::{InstalledSageAppSnapshot, SageAppPackageManifest},
};

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

pub fn snapshots_root(install_dir: &Path) -> PathBuf {
    install_dir.join("snapshots")
}

pub fn snapshot_dir_for_hash(install_dir: &Path, manifest_hash: &str) -> PathBuf {
    snapshots_root(install_dir).join(manifest_hash)
}

pub async fn fetch_url_bytes(url: &str, max_size: u64) -> AnyResult<Vec<u8>> {
    let response = reqwest::Client::new()
        .get(url)
        .send()
        .await
        .with_context(|| format!("failed to GET {}", url))?;

    if !response.status().is_success() {
        return Err(anyhow!("GET {} returned HTTP {}", url, response.status()));
    }

    let bytes = response.bytes().await?;
    let len = bytes.len() as u64;

    if len > max_size {
        return Err(anyhow!(
            "response from {} exceeds size limit {}",
            url,
            max_size
        ));
    }

    Ok(bytes.to_vec())
}

pub async fn fetch_url_manifest(
    manifest_url: &str,
) -> AnyResult<(SageAppPackageManifest, String)> {
    let bytes = fetch_url_bytes(manifest_url, MAX_MANIFEST_SIZE_BYTES).await?;
    let manifest_hash = sha256_hex(&bytes);

    let manifest: SageAppPackageManifest =
        serde_json::from_slice(&bytes).context("failed to parse sage-manifest.json")?;

    validate_package_manifest(&manifest)?;

    Ok((manifest, manifest_hash))
}

pub fn derive_manifest_url(app_url: &str) -> AnyResult<String> {
    let parsed = Url::parse(app_url).context("invalid normalized app URL")?;
    let manifest = parsed
        .join("sage-manifest.json")
        .context("failed to derive manifest URL")?;
    Ok(manifest.to_string())
}

pub fn build_file_url(app_url: &str, relative_path: &str) -> AnyResult<String> {
    let base = Url::parse(app_url).context("invalid normalized app URL")?;
    let file = base
        .join(relative_path)
        .with_context(|| format!("failed to build file URL for {}", relative_path))?;
    Ok(file.to_string())
}

pub async fn download_url_snapshot(
    install_dir: &Path,
    app_url: &str,
    manifest: &SageAppPackageManifest,
    manifest_hash: &str,
) -> AnyResult<InstalledSageAppSnapshot> {
    let total_bytes = validate_package_manifest(manifest)?;

    let snapshots_root = snapshots_root(install_dir);
    fs::create_dir_all(&snapshots_root)?;

    let final_dir = snapshot_dir_for_hash(install_dir, manifest_hash);
    if final_dir.exists() {
        return Ok(InstalledSageAppSnapshot {
            manifest_hash: manifest_hash.to_string(),
            snapshot_dir: final_dir.to_string_lossy().to_string(),
            total_bytes,
            manifest: manifest.clone(),
        });
    }

    let temp_dir = snapshots_root.join(format!(".tmp-{}", manifest_hash));
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir)?;
    }
    fs::create_dir_all(&temp_dir)?;

    let result = async {
        let mut actual_total: u64 = 0;

        for file in &manifest.files {
            let file_url = build_file_url(app_url, &file.path)?;
            let bytes = fetch_url_bytes(&file_url, MAX_APP_TOTAL_SIZE_BYTES).await?;

            let actual_size = bytes.len() as u64;
            if actual_size != file.size {
                return Err(anyhow!(
                    "size mismatch for {}: expected {}, got {}",
                    file.path,
                    file.size,
                    actual_size
                ));
            }

            let actual_sha256 = sha256_hex(&bytes);
            if actual_sha256 != file.sha256.to_ascii_lowercase() {
                return Err(anyhow!(
                    "sha256 mismatch for {}: expected {}, got {}",
                    file.path,
                    file.sha256,
                    actual_sha256
                ));
            }

            actual_total = actual_total
                .checked_add(actual_size)
                .ok_or_else(|| anyhow!("snapshot total size overflow"))?;

            if actual_total > MAX_APP_TOTAL_SIZE_BYTES {
                return Err(anyhow!(
                    "snapshot total size {} exceeds limit {}",
                    actual_total,
                    MAX_APP_TOTAL_SIZE_BYTES
                ));
            }

            let out_path = temp_dir.join(&file.path);
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&out_path, &bytes)?;
        }

        if temp_dir.join("index.html").is_file() == false {
            return Err(anyhow!("snapshot is missing root index.html"));
        }

        let manifest_path = temp_dir.join("sage-manifest.json");
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(manifest)
                .map_err(|err| anyhow!("failed to serialize manifest: {err}"))?
                + "\n",
        )?;

        fs::rename(&temp_dir, &final_dir).with_context(|| {
            format!(
                "failed to move snapshot {} to {}",
                temp_dir.display(),
                final_dir.display()
            )
        })?;

        Ok::<(), anyhow::Error>(())
    }
        .await;

    if result.is_err() && temp_dir.exists() {
        let _ = fs::remove_dir_all(&temp_dir);
    }

    result?;

    Ok(InstalledSageAppSnapshot {
        manifest_hash: manifest_hash.to_string(),
        snapshot_dir: final_dir.to_string_lossy().to_string(),
        total_bytes,
        manifest: manifest.clone(),
    })
}

pub fn read_snapshot_file(snapshot_dir: &Path, request_path: &str) -> AnyResult<PathBuf> {
    let target = if request_path.is_empty() || request_path == "/" || request_path == "/index.html" {
        snapshot_dir.join("index.html")
    } else {
        let trimmed = request_path.trim_start_matches('/');

        if trimmed.contains("..") {
            return Err(anyhow!("invalid snapshot path"));
        }

        snapshot_dir.join(trimmed)
    };

    let canonical_snapshot_dir = snapshot_dir.canonicalize()?;
    let canonical_target = target
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", target.display()))?;

    if !canonical_target.starts_with(&canonical_snapshot_dir) {
        return Err(anyhow!("requested snapshot path escapes snapshot dir"));
    }

    if !canonical_target.is_file() {
        return Err(anyhow!("requested snapshot file does not exist"));
    }

    Ok(canonical_target)
}
