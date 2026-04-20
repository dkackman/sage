use sage_lib::apps::manifest::{
    validate_manifest_file_path, validate_sha256_hex,
};

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
