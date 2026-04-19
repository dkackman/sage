use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), String> {
    if !src.is_dir() {
        return Err(format!("missing directory: {}", src.display()));
    }

    fs::create_dir_all(dst)
        .map_err(|err| format!("failed to create {}: {err}", dst.display()))?;

    for entry in fs::read_dir(src)
        .map_err(|err| format!("failed to read {}: {err}", src.display()))?
    {
        let entry = entry
            .map_err(|err| format!("failed to read entry in {}: {err}", src.display()))?;

        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        let file_type = entry
            .file_type()
            .map_err(|err| format!("failed to stat {}: {err}", src_path.display()))?;

        if file_type.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else if file_type.is_file() {
            if let Some(parent) = dst_path.parent() {
                fs::create_dir_all(parent).map_err(|err| {
                    format!("failed to create parent {}: {err}", parent.display())
                })?;
            }

            fs::copy(&src_path, &dst_path).map_err(|err| {
                format!(
                    "failed to copy {} -> {}: {err}",
                    src_path.display(),
                    dst_path.display()
                )
            })?;
        }
    }

    Ok(())
}

fn copy_file_required(src: &Path, dst: &Path, label: &str) -> Result<(), String> {
    if !src.is_file() {
        return Err(format!("missing {label} at {}", src.display()));
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

fn remove_manifest_files(dir: &Path) -> Result<(), String> {
    if !dir.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(dir)
        .map_err(|err| format!("failed to read {}: {err}", dir.display()))?
    {
        let entry = entry
            .map_err(|err| format!("failed to read entry in {}: {err}", dir.display()))?;
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };

        if name.starts_with("sage-manifest") && name.ends_with(".json") {
            fs::remove_file(&path)
                .map_err(|err| format!("failed to remove {}: {err}", path.display()))?;
        }
    }

    Ok(())
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

    if shared_dir.is_dir() {
        copy_dir_all(shared_dir, out_dir)?;
    }

    copy_dir_all(source_dir, out_dir)?;
    remove_manifest_files(out_dir)?;

    if let Some(manifest_file_name) = manifest_file_name {
        copy_file_required(
            &source_dir.join(manifest_file_name),
            &out_dir.join("sage-manifest.json"),
            &format!("manifest {manifest_file_name}"),
        )?;
    }

    copy_file_required(
        &sdk_dist.join("runtime-bridge.js"),
        &out_dir.join("bridge.js"),
        "SDK runtime bridge build output",
    )?;

    copy_file_required(
        &sdk_dist.join("index.js"),
        &out_dir.join("sdk.js"),
        "SDK index build output",
    )?;

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
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").map_err(|err| {
        format!("CARGO_MANIFEST_DIR not set: {err}")
    })?);

    let builtin_root = manifest_dir.join("builtin-apps");

    let shared_dir = builtin_root.join("shared");
    let test_src_dir = builtin_root.join("test-apps-src");
    let runtime_src_dir = builtin_root.join("runtime-apps-src");

    let dist_root = builtin_root.join("dist");
    let test_out_dir = dist_root.join("test-apps");
    let runtime_out_dir = dist_root.join("runtime-apps");

    let workspace_root = manifest_dir
        .parent()
        .ok_or_else(|| "src-tauri should have a parent directory".to_string())?;

    let sdk_dist = workspace_root
        .join("packages")
        .join("sage-app-sdk")
        .join("dist");

    println!("cargo:rerun-if-changed={}", shared_dir.display());
    println!("cargo:rerun-if-changed={}", test_src_dir.display());
    println!("cargo:rerun-if-changed={}", runtime_src_dir.display());
    println!("cargo:rerun-if-changed={}", sdk_dist.display());

    if dist_root.exists() {
        fs::remove_dir_all(&dist_root)
            .map_err(|err| format!("failed to remove {}: {err}", dist_root.display()))?;
    }

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
