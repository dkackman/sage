use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result as AnyResult};
use zip::ZipArchive;

use crate::apps::{
    lifecycle::hash_string,
    lifecycle::manifest::validate_package_manifest,
    types::{InstalledSageAppSnapshot, SageAppPackageManifest},
};

const MANIFEST_FILE_NAME: &str = "sage-manifest.json";

pub fn unzip_to_dir(zip_path: &Path, out_dir: &Path) -> AnyResult<()> {
    let file = fs::File::open(zip_path)
        .with_context(|| format!("failed to open zip {}", zip_path.display()))?;
    let mut archive = ZipArchive::new(file).context("failed to read zip archive")?;

    if out_dir.exists() {
        fs::remove_dir_all(out_dir)?;
    }
    fs::create_dir_all(out_dir)?;

    archive.extract(out_dir).context("failed to extract zip archive")?;
    Ok(())
}

pub fn detect_package_root(unpack_dir: &Path) -> AnyResult<PathBuf> {
    let direct_manifest = unpack_dir.join(MANIFEST_FILE_NAME);
    if direct_manifest.is_file() {
        return Ok(unpack_dir.to_path_buf());
    }

    let mut dirs = fs::read_dir(unpack_dir)?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| entry.file_type().ok().filter(|ft| ft.is_dir()).map(|_| entry.path()))
        .collect::<Vec<_>>();

    dirs.sort();

    for dir in dirs {
        if dir.join(MANIFEST_FILE_NAME).is_file() {
            return Ok(dir);
        }
    }

    anyhow::bail!("could not find {}", MANIFEST_FILE_NAME)
}

pub fn read_manifest(package_root: &Path) -> AnyResult<SageAppPackageManifest> {
    let manifest_path = package_root.join(MANIFEST_FILE_NAME);
    let manifest_text = fs::read_to_string(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let manifest: SageAppPackageManifest =
        serde_json::from_str(&manifest_text).context("failed to parse manifest")?;
    validate_package_manifest(&manifest)?;
    Ok(manifest)
}

pub fn validate_package_structure(package_root: &Path) -> AnyResult<()> {
    let manifest = read_manifest(package_root)?;

    for file in &manifest.files {
        let path = package_root.join(&file.path);
        if !path.is_file() {
            anyhow::bail!("manifest file missing from package: {}", file.path);
        }

        let bytes = fs::read(&path)
            .with_context(|| format!("failed to read package file {}", path.display()))?;

        let actual_hash = hash_string(&String::from_utf8_lossy(&bytes));
        if actual_hash != file.sha256 {
            anyhow::bail!("sha256 mismatch for {}", file.path);
        }

        let actual_size = u64::try_from(bytes.len()).context("file too large")?;
        if actual_size != file.size {
            anyhow::bail!("size mismatch for {}", file.path);
        }
    }

    Ok(())
}

pub fn prepare_zip_snapshot(
    package_root: &Path,
    install_dir: &Path,
    manifest: &SageAppPackageManifest,
) -> AnyResult<InstalledSageAppSnapshot> {
    let snapshot_dir = install_dir.join("active");
    if snapshot_dir.exists() {
        fs::remove_dir_all(&snapshot_dir)?;
    }
    fs::create_dir_all(&snapshot_dir)?;

    let mut total_bytes = 0_u64;

    for file in &manifest.files {
        let src = package_root.join(&file.path);
        let dst = snapshot_dir.join(&file.path);

        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::copy(&src, &dst)
            .with_context(|| format!("failed to copy {} to {}", src.display(), dst.display()))?;
        total_bytes = total_bytes.saturating_add(file.size);
    }

    Ok(InstalledSageAppSnapshot {
        manifest_hash: hash_string(&serde_json::to_string(manifest)?),
        snapshot_dir: snapshot_dir.to_string_lossy().to_string(),
        total_bytes,
        manifest: manifest.clone(),
    })
}
