use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn files_equal(src: &Path, dst: &Path) -> Result<bool, String> {
    if !src.is_file() || !dst.is_file() {
        return Ok(false);
    }

    let src_meta =
        fs::metadata(src).map_err(|err| format!("failed to stat {}: {err}", src.display()))?;
    let dst_meta =
        fs::metadata(dst).map_err(|err| format!("failed to stat {}: {err}", dst.display()))?;

    if src_meta.len() != dst_meta.len() {
        return Ok(false);
    }

    let src_bytes =
        fs::read(src).map_err(|err| format!("failed to read {}: {err}", src.display()))?;
    let dst_bytes =
        fs::read(dst).map_err(|err| format!("failed to read {}: {err}", dst.display()))?;

    Ok(src_bytes == dst_bytes)
}

fn copy_file_if_changed(src: &Path, dst: &Path, label: &str) -> Result<(), String> {
    if !src.is_file() {
        return Err(format!("missing {label} at {}", src.display()));
    }

    if files_equal(src, dst)? {
        return Ok(());
    }

    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }

    fs::copy(src, dst).map_err(|err| {
        format!(
            "failed to copy {} -> {}: {err}",
            src.display(),
            dst.display()
        )
    })?;

    Ok(())
}

fn collect_files_recursive(
    root: &Path,
    current: &Path,
    out: &mut Vec<PathBuf>,
) -> Result<(), String> {
    for entry in fs::read_dir(current)
        .map_err(|err| format!("failed to read {}: {err}", current.display()))?
    {
        let entry = entry
            .map_err(|err| format!("failed to read entry in {}: {err}", current.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|err| format!("failed to stat {}: {err}", path.display()))?;

        if file_type.is_dir() {
            collect_files_recursive(root, &path, out)?;
        } else if file_type.is_file() {
            let rel = path
                .strip_prefix(root)
                .map_err(|err| {
                    format!(
                        "failed to strip prefix {} from {}: {err}",
                        root.display(),
                        path.display()
                    )
                })?
                .to_path_buf();
            out.push(rel);
        }
    }

    Ok(())
}

fn copy_dir_all_if_changed(
    src: &Path,
    dst: &Path,
    expected_files: &mut BTreeSet<PathBuf>,
) -> Result<(), String> {
    if !src.is_dir() {
        return Err(format!("missing directory: {}", src.display()));
    }

    fs::create_dir_all(dst)
        .map_err(|err| format!("failed to create {}: {err}", dst.display()))?;

    let mut rel_files = Vec::new();
    collect_files_recursive(src, src, &mut rel_files)?;

    for rel in rel_files {
        let src_path = src.join(&rel);
        let dst_path = dst.join(&rel);
        copy_file_if_changed(&src_path, &dst_path, "built app file")?;
        expected_files.insert(rel);
    }

    Ok(())
}

fn remove_unexpected_files(dir: &Path, expected_files: &BTreeSet<PathBuf>) -> Result<(), String> {
    if !dir.is_dir() {
        return Ok(());
    }

    let mut existing_files = Vec::new();
    collect_files_recursive(dir, dir, &mut existing_files)?;

    for rel in existing_files {
        if !expected_files.contains(&rel) {
            let path = dir.join(&rel);
            fs::remove_file(&path)
                .map_err(|err| format!("failed to remove {}: {err}", path.display()))?;
        }
    }

    remove_empty_dirs_recursively(dir)?;
    Ok(())
}

fn remove_empty_dirs_recursively(dir: &Path) -> Result<bool, String> {
    if !dir.is_dir() {
        return Ok(false);
    }

    let mut is_empty = true;

    for entry in fs::read_dir(dir)
        .map_err(|err| format!("failed to read {}: {err}", dir.display()))?
    {
        let entry =
            entry.map_err(|err| format!("failed to read entry in {}: {err}", dir.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|err| format!("failed to stat {}: {err}", path.display()))?;

        if file_type.is_dir() {
            let child_empty = remove_empty_dirs_recursively(&path)?;
            if child_empty {
                fs::remove_dir(&path)
                    .map_err(|err| format!("failed to remove {}: {err}", path.display()))?;
            } else {
                is_empty = false;
            }
        } else {
            is_empty = false;
        }
    }

    Ok(is_empty)
}

fn finalize_built_app(
    shared_dir: &Path,
    source_dir: &Path,
    out_dir: &Path,
    manifest_file_name: Option<&str>,
    sdk_dist: &Path,
) -> Result<(), String> {
    fs::create_dir_all(out_dir)
        .map_err(|err| format!("failed to create {}: {err}", out_dir.display()))?;

    let mut expected_files = BTreeSet::<PathBuf>::new();

    if shared_dir.is_dir() {
        copy_dir_all_if_changed(shared_dir, out_dir, &mut expected_files)?;
    }

    copy_dir_all_if_changed(source_dir, out_dir, &mut expected_files)?;

    if let Some(manifest_file_name) = manifest_file_name {
        let rel = PathBuf::from("sage-manifest.json");
        copy_file_if_changed(
            &source_dir.join(manifest_file_name),
            &out_dir.join(&rel),
            &format!("manifest {manifest_file_name}"),
        )?;
        expected_files.insert(rel);

        for legacy_name in [
            "sage-manifest.json",
            "sage-manifest.persistent.json",
            "sage-manifest.incognito.json",
        ] {
            let rel = PathBuf::from(legacy_name);
            if rel != PathBuf::from("sage-manifest.json") {
                let path = out_dir.join(&rel);
                if path.is_file() {
                    fs::remove_file(&path).map_err(|err| {
                        format!("failed to remove {}: {err}", path.display())
                    })?;
                }
            }
        }
    }

    let bridge_rel = PathBuf::from("bridge.js");
    copy_file_if_changed(
        &sdk_dist.join("runtime-bridge.js"),
        &out_dir.join(&bridge_rel),
        "SDK runtime bridge build output",
    )?;
    expected_files.insert(bridge_rel);

    let sdk_rel = PathBuf::from("sdk.js");
    copy_file_if_changed(
        &sdk_dist.join("index.js"),
        &out_dir.join(&sdk_rel),
        "SDK index build output",
    )?;
    expected_files.insert(sdk_rel);

    remove_unexpected_files(out_dir, &expected_files)?;
    Ok(())
}

#[derive(Clone, Copy)]
struct TestVariant {
    out_dir_name: &'static str,
    manifest_file_name: &'static str,
}

#[derive(Clone, Copy)]
struct TestGroup {
    source_dir_name: &'static str,
    variants: &'static [TestVariant],
}

#[derive(Clone, Copy)]
struct RuntimeApp {
    source_dir_name: &'static str,
    out_dir_name: &'static str,
}

const TEST_BUILD_PLAN: &[TestGroup] = &[
    TestGroup {
        source_dir_name: "sage-storage-isolation",
        variants: &[
            TestVariant {
                out_dir_name: "sage-storage-isolation-persistent",
                manifest_file_name: "sage-manifest.persistent.json",
            },
            TestVariant {
                out_dir_name: "sage-storage-isolation-incognito",
                manifest_file_name: "sage-manifest.incognito.json",
            },
        ],
    },
    TestGroup {
        source_dir_name: "storage-persistence",
        variants: &[
            TestVariant {
                out_dir_name: "storage-persistence-persistent",
                manifest_file_name: "sage-manifest.persistent.json",
            },
            TestVariant {
                out_dir_name: "storage-persistence-incognito",
                manifest_file_name: "sage-manifest.incognito.json",
            },
            TestVariant {
                out_dir_name: "storage-clear-persistent",
                manifest_file_name: "sage-manifest.persistent.json",
            },
        ],
    },
    TestGroup {
        source_dir_name: "network-allow-a",
        variants: &[TestVariant {
            out_dir_name: "network-allow-a",
            manifest_file_name: "sage-manifest.json",
        }],
    },
    TestGroup {
        source_dir_name: "network-allow-b",
        variants: &[TestVariant {
            out_dir_name: "network-allow-b",
            manifest_file_name: "sage-manifest.json",
        }],
    },
];

const RUNTIME_BUILD_PLAN: &[RuntimeApp] = &[RuntimeApp {
    source_dir_name: "storage-clear-probe",
    out_dir_name: "storage-clear-probe",
}];

pub fn build_builtin_apps() -> Result<(), String> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let builtin_root = manifest_dir.join("builtin-apps");

    let shared_dir = builtin_root.join("shared");
    let test_src_dir = builtin_root.join("test-apps-src");
    let runtime_src_dir = builtin_root.join("runtime-apps-src");

    let dist_root = builtin_root.join("dist");
    let test_out_dir = dist_root.join("test-apps");
    let runtime_out_dir = dist_root.join("runtime-apps");

    let workspace_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .ok_or_else(|| "crates/sage-apps should have workspace root above it".to_string())?;

    let sdk_dist = workspace_root
        .join("packages")
        .join("sage-app-sdk")
        .join("dist");

    println!("cargo:rerun-if-changed={}", shared_dir.display());
    println!("cargo:rerun-if-changed={}", test_src_dir.display());
    println!("cargo:rerun-if-changed={}", runtime_src_dir.display());
    println!("cargo:rerun-if-changed={}", sdk_dist.display());

    fs::create_dir_all(&test_out_dir)
        .map_err(|err| format!("failed to create {}: {err}", test_out_dir.display()))?;
    fs::create_dir_all(&runtime_out_dir)
        .map_err(|err| format!("failed to create {}: {err}", runtime_out_dir.display()))?;

    for group in TEST_BUILD_PLAN {
        for variant in group.variants {
            finalize_built_app(
                &shared_dir,
                &test_src_dir.join(group.source_dir_name),
                &test_out_dir.join(variant.out_dir_name),
                Some(variant.manifest_file_name),
                &sdk_dist,
            )?;
        }
    }

    for runtime_app in RUNTIME_BUILD_PLAN {
        finalize_built_app(
            &shared_dir,
            &runtime_src_dir.join(runtime_app.source_dir_name),
            &runtime_out_dir.join(runtime_app.out_dir_name),
            None,
            &sdk_dist,
        )?;
    }

    Ok(())
}
