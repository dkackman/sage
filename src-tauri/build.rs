use glob::glob;
use std::env;

/// Adds a temporary workaround for an issue with the Rust compiler and Android
/// in `x86_64` devices: <https://github.com/rust-lang/rust/issues/109717>.
/// The workaround comes from: <https://github.com/smartvaults/smartvaults/blob/827805a989561b78c0ea5b41f2c1c9e9e59545e0/bindings/smartvaults-sdk-ffi/build.rs>
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

fn main() {
    setup_x86_64_android_workaround();

    // The webauthn plugin links Swift code (via swift-rs) that uses async/await,
    // which requires libswift_Concurrency. For deployment targets below macOS 15,
    // this is a back-deploy library in the Xcode toolchain rather than /usr/lib/swift/.
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("xcode-select")
            .arg("--print-path")
            .output()
        {
            let xcode_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let swift_lib = format!(
                "{xcode_path}/Toolchains/XcodeDefault.xctoolchain/usr/lib/swift-5.5/macosx"
            );
            if std::path::Path::new(&swift_lib).exists() {
                println!("cargo:rustc-link-arg=-Wl,-rpath,{swift_lib}");
            }
        }
    }

    tauri_build::build();
}
