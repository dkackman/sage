use sage_lib::apps::lifecycle::limits::{
    MAX_APP_FILE_COUNT, MAX_APP_TOTAL_SIZE_BYTES,
};
use sage_lib::apps::lifecycle::manifest::{
    validate_manifest_file_path, validate_manifest_files, validate_package_manifest,
    validate_sha256_hex,
};
use sage_lib::apps::types::{
    SageAppManifestFile, SageAppPackageManifest, SageRequestedCapabilities,
    SageRequestedNetworkPermissions, SageRequestedNetworkWhitelist,
    SageRequestedPermissions,
};

fn sample_manifest_file(path: &str, size: u64) -> SageAppManifestFile {
    SageAppManifestFile {
        path: path.to_string(),
        sha256: "a".repeat(64),
        size,
    }
}

fn empty_permissions() -> SageRequestedPermissions {
    SageRequestedPermissions {
        network: SageRequestedNetworkPermissions {
            whitelist: SageRequestedNetworkWhitelist {
                required: vec![],
                optional: vec![],
            },
        },
        capabilities: SageRequestedCapabilities {
            required: vec![],
            optional: vec![],
        },
    }
}

fn sample_manifest() -> SageAppPackageManifest {
    SageAppPackageManifest {
        name: "Test App".to_string(),
        version: "1.0.0".to_string(),
        permissions: empty_permissions(),
        files: vec![sample_manifest_file("dist/index.html", 123)],
        entry: Some("dist/index.html".to_string()),
        icon: Some("dist/icon.png".to_string()),
    }
}

#[test]
fn validate_manifest_file_path_accepts_normal_relative_path() {
    validate_manifest_file_path("dist/index.html").unwrap();
}

#[test]
fn validate_manifest_file_path_rejects_absolute_path() {
    assert!(validate_manifest_file_path("/etc/passwd").is_err());
}

#[test]
fn validate_manifest_file_path_rejects_parent_traversal() {
    assert!(validate_manifest_file_path("../secret.txt").is_err());
}

#[test]
fn validate_manifest_file_path_rejects_current_dir_segment() {
    assert!(validate_manifest_file_path("./index.html").is_err());
    assert!(validate_manifest_file_path("dist/./index.html").is_err());
}

#[test]
fn validate_manifest_file_path_rejects_empty_segment() {
    assert!(validate_manifest_file_path("dist//index.html").is_err());
}

#[test]
fn validate_manifest_file_path_rejects_backslashes() {
    assert!(validate_manifest_file_path(r"dist\index.html").is_err());
}

#[test]
fn validate_sha256_hex_accepts_valid_hash() {
    validate_sha256_hex(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    )
        .unwrap();
}

#[test]
fn validate_sha256_hex_rejects_invalid_hash() {
    assert!(validate_sha256_hex("not-a-sha").is_err());
}

#[test]
fn validate_manifest_files_rejects_empty_list() {
    let err = validate_manifest_files(&[]).unwrap_err();
    assert!(err.to_string().contains("cannot be empty"));
}

#[test]
fn validate_manifest_files_rejects_duplicate_paths() {
    let files = vec![
        sample_manifest_file("dist/index.html", 1),
        sample_manifest_file("dist/index.html", 2),
    ];

    let err = validate_manifest_files(&files).unwrap_err();
    assert!(err.to_string().contains("duplicate manifest file path"));
}

#[test]
fn validate_manifest_files_rejects_invalid_nested_path() {
    let files = vec![sample_manifest_file("dist//index.html", 1)];

    let err = validate_manifest_files(&files).unwrap_err();
    assert!(err.to_string().contains("manifest file path is invalid"));
}

#[test]
fn validate_manifest_files_rejects_file_count_over_limit() {
    let files: Vec<_> = (0..=MAX_APP_FILE_COUNT)
        .map(|i| sample_manifest_file(&format!("dist/file-{i}.txt"), 1))
        .collect();

    let err = validate_manifest_files(&files).unwrap_err();
    assert!(err.to_string().contains("exceeds limit"));
}

#[test]
fn validate_manifest_files_rejects_total_size_over_limit() {
    let files = vec![
        sample_manifest_file("dist/a.bin", MAX_APP_TOTAL_SIZE_BYTES),
        sample_manifest_file("dist/b.bin", 1),
    ];

    let err = validate_manifest_files(&files).unwrap_err();
    assert!(err.to_string().contains("manifest total size"));
    assert!(err.to_string().contains("exceeds limit"));
}

#[test]
fn validate_manifest_files_returns_total_size_when_valid() {
    let files = vec![
        sample_manifest_file("dist/index.html", 100),
        sample_manifest_file("dist/icon.png", 23),
    ];

    let total = validate_manifest_files(&files).unwrap();
    assert_eq!(total, 123);
}

#[test]
fn validate_package_manifest_rejects_blank_name() {
    let mut manifest = sample_manifest();
    manifest.name = "   ".to_string();

    let err = validate_package_manifest(&manifest).unwrap_err();
    assert!(err.to_string().contains("manifest name cannot be empty"));
}

#[test]
fn validate_package_manifest_rejects_blank_version() {
    let mut manifest = sample_manifest();
    manifest.version = "   ".to_string();

    let err = validate_package_manifest(&manifest).unwrap_err();
    assert!(err.to_string().contains("manifest version cannot be empty"));
}

#[test]
fn validate_package_manifest_rejects_invalid_requested_capability() {
    let mut manifest = sample_manifest();
    manifest.permissions.capabilities.required =
        vec!["definitely.not.a.real.capability".to_string()];

    let err = validate_package_manifest(&manifest).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("unknown capability") || msg.contains("definitely.not.a.real.capability"),
        "unexpected error: {msg}"
    );
}

#[test]
fn validate_package_manifest_returns_total_size_when_valid() {
    let manifest = sample_manifest();
    let total = validate_package_manifest(&manifest).unwrap();
    assert_eq!(total, 123);
}
