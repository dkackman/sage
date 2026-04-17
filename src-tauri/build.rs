use glob::glob;
use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

fn setup_x86_64_android_workaround() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").expect("CARGO_CFG_TARGET_OS not set");
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").expect("CARGO_CFG_TARGET_ARCH not set");
    if target_arch == "x86_64" && target_os == "android" {
        let android_ndk_home = env::var("ANDROID_NDK_HOME").expect("ANDROID_NDK_HOME not set");
        let build_os = match env::consts::OS {
            "linux" => "linux",
            "macos" => "darwin",
            "windows" => "windows",
            _ => panic!(
                "Unsupported OS. You must use either Linux, MacOS or Windows to build the crate."
            ),
        };
        let linux_x86_64_lib_pattern = format!(
            "{android_ndk_home}/toolchains/llvm/prebuilt/{build_os}-x86_64/lib*/clang/**/lib/linux/"
        );
        match glob(&linux_x86_64_lib_pattern).expect("glob failed").last() {
            Some(Ok(path)) => {
                println!("cargo:rustc-link-search={}", path.to_string_lossy());
                println!("cargo:rustc-link-lib=static=clang_rt.builtins-x86_64-android");
            }
            _ => panic!(
                "Path not found: {linux_x86_64_lib_pattern}. Try setting a different ANDROID_NDK_HOME."
            ),
        }
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> io::Result<()> {
    if !src.exists() {
        return Ok(());
    }

    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let from = entry.path();
        let to = dst.join(entry.file_name());

        if file_type.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else if file_type.is_file() {
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&from, &to)?;
        }
    }

    Ok(())
}

fn remove_dir_if_exists(path: &Path) -> io::Result<()> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}

fn list_subdirs(path: &Path) -> io::Result<Vec<PathBuf>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let mut dirs = Vec::new();

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            dirs.push(entry.path());
        }
    }

    dirs.sort();
    Ok(dirs)
}

fn print_rerun_if_changed_recursive(path: &Path) -> io::Result<()> {
    if !path.exists() {
        return Ok(());
    }

    println!("cargo:rerun-if-changed={}", path.display());

    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            print_rerun_if_changed_recursive(&entry.path())?;
        }
    }

    Ok(())
}

fn copy_file(src: &Path, dst: &Path) -> io::Result<()> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(src, dst)?;
    Ok(())
}

fn build_test_apps() -> io::Result<()> {
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));
    let workspace_root = manifest_dir
        .parent()
        .expect("src-tauri should have a parent directory")
        .to_path_buf();

    let src_root = workspace_root.join("src").join("test-apps-src");
    let shared_dir = src_root.join("_shared");
    let out_root = workspace_root.join("src").join("test-apps");

    let sdk_dist_dir = workspace_root
        .join("packages")
        .join("sage-app-sdk")
        .join("dist");

    let sdk_runtime_bridge = sdk_dist_dir.join("runtime-bridge.js");
    let sdk_index = sdk_dist_dir.join("index.js");

    print_rerun_if_changed_recursive(&src_root)?;
    print_rerun_if_changed_recursive(&sdk_dist_dir)?;

    if !src_root.exists() {
        return Ok(());
    }

    if !sdk_runtime_bridge.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "missing SDK runtime bridge build output at {}. Run `pnpm --filter @sage-app/sdk build` first.",
                sdk_runtime_bridge.display()
            ),
        ));
    }

    if !sdk_index.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "missing SDK index build output at {}. Run `pnpm --filter @sage-app/sdk build` first.",
                sdk_index.display()
            ),
        ));
    }

    fs::create_dir_all(&out_root)?;

    let source_app_dirs = list_subdirs(&src_root)?;
    let mut expected_output_names = Vec::new();

    for app_src_dir in source_app_dirs {
        let app_name = app_src_dir
            .file_name()
            .and_then(|s| s.to_str())
            .expect("invalid test app source directory name")
            .to_string();

        if app_name == "_shared" {
            continue;
        }

        expected_output_names.push(app_name.clone());

        let app_out_dir = out_root.join(&app_name);

        remove_dir_if_exists(&app_out_dir)?;
        fs::create_dir_all(&app_out_dir)?;

        copy_dir_recursive(&shared_dir, &app_out_dir)?;
        copy_dir_recursive(&app_src_dir, &app_out_dir)?;

        copy_file(&sdk_runtime_bridge, &app_out_dir.join("bridge.js"))?;
        copy_file(&sdk_index, &app_out_dir.join("sdk.js"))?;
    }

    for existing_out_dir in list_subdirs(&out_root)? {
        let out_name = existing_out_dir
            .file_name()
            .and_then(|s| s.to_str())
            .expect("invalid generated test app directory name")
            .to_string();

        if !expected_output_names.iter().any(|name| name == &out_name) {
            fs::remove_dir_all(&existing_out_dir)?;
        }
    }

    Ok(())
}

fn main() {
    setup_x86_64_android_workaround();
    build_test_apps().expect("failed to build materialized test apps");
    tauri_build::build();
}
