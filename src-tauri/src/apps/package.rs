use std::{
    fs, io,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context, Result as AnyResult};
use zip::ZipArchive;

use crate::apps::{
    install::hash_string,
    manifest::validate_package_manifest,
    types::{InstalledSageAppSnapshot, SageAppPackageManifest},
};

pub fn should_ignore_root_entry(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
        return false;
    };

    name == "__MACOSX" || name == ".DS_Store"
}

pub fn copy_dir_all(src: &Path, dst: &Path) -> AnyResult<()> {
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

pub fn unzip_to_dir(zip_path: &Path, out_dir: &Path) -> AnyResult<()> {
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

pub fn detect_package_root(unpack_dir: &Path) -> AnyResult<PathBuf> {
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

pub fn read_manifest(package_root: &Path) -> AnyResult<SageAppPackageManifest> {
    let manifest_path = package_root.join("manifest.json");
    let text = fs::read_to_string(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;

    let manifest: SageAppPackageManifest =
        serde_json::from_str(&text).context("failed to parse manifest.json")?;

    validate_manifest(&manifest)?;
    Ok(manifest)
}

pub fn validate_manifest(manifest: &SageAppPackageManifest) -> AnyResult<()> {
    validate_package_manifest(manifest).map(|_| ())
}

pub fn validate_package_structure(package_root: &Path) -> AnyResult<()> {
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

pub fn prepare_zip_snapshot(
    package_root: &Path,
    install_dir: &Path,
    manifest: &SageAppPackageManifest,
) -> AnyResult<InstalledSageAppSnapshot> {
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

    Ok(InstalledSageAppSnapshot {
        manifest_hash,
        snapshot_dir: snapshot_dir.to_string_lossy().to_string(),
        total_bytes,
        manifest: manifest.clone(),
    })
}
