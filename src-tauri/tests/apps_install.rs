use sage_lib::apps::lifecycle::install::{
    manifest_entry_file, manifest_icon_file, normalize_app_url,
};
use sage_lib::apps::types::{
    SageAppManifestFile, SageAppPackageManifest, SageNetworkPermissionTarget,
    SageRequestedCapabilities, SageRequestedNetworkPermissions,
    SageRequestedNetworkWhitelist, SageRequestedPermissions,
};

fn requested_permissions() -> SageRequestedPermissions {
    SageRequestedPermissions {
        network: SageRequestedNetworkPermissions {
            whitelist: SageRequestedNetworkWhitelist {
                required: vec![SageNetworkPermissionTarget {
                    scheme: "https".to_string(),
                    host: "required.example.com".to_string(),
                }],
                optional: vec![SageNetworkPermissionTarget {
                    scheme: "wss".to_string(),
                    host: "optional.example.com".to_string(),
                }],
            },
        },
        capabilities: SageRequestedCapabilities {
            required: vec!["wallet.send_xch".to_string()],
            optional: vec!["persistent_storage".to_string()],
        },
    }
}

fn sample_manifest() -> SageAppPackageManifest {
    SageAppPackageManifest {
        name: "Test App".to_string(),
        version: "1.0.0".to_string(),
        permissions: requested_permissions(),
        files: vec![SageAppManifestFile {
            path: "index.html".to_string(),
            sha256: "a".repeat(64),
            size: 1,
        }],
        entry: Some("entry.html".to_string()),
        icon: Some("icon.svg".to_string()),
    }
}

#[test]
fn normalize_app_url_keeps_https_and_adds_trailing_slash() {
    let out = normalize_app_url("https://example.com/app").unwrap();
    assert_eq!(out, "https://example.com/app/");
}

#[test]
fn normalize_app_url_strips_query_and_fragment() {
    let out = normalize_app_url("https://example.com/app?x=1#frag").unwrap();
    assert_eq!(out, "https://example.com/app/");
}

#[test]
fn normalize_app_url_allows_localhost_http() {
    let out = normalize_app_url("http://localhost:4173").unwrap();
    assert_eq!(out, "http://localhost:4173/");
}

#[test]
fn normalize_app_url_allows_loopback_http() {
    assert_eq!(
        normalize_app_url("http://127.0.0.1:4173").unwrap(),
        "http://127.0.0.1:4173/"
    );
}

#[test]
fn normalize_app_url_rejects_non_local_http() {
    let err = normalize_app_url("http://example.com/app")
        .unwrap_err()
        .to_string();
    assert!(err.contains("requires HTTPS"));
}

#[test]
fn normalize_app_url_rejects_unsupported_scheme() {
    let err = normalize_app_url("ftp://example.com/app")
        .unwrap_err()
        .to_string();
    assert!(err.contains("unsupported app URL scheme"));
}

#[test]
fn manifest_entry_file_uses_explicit_entry() {
    let manifest = sample_manifest();
    assert_eq!(manifest_entry_file(&manifest), "entry.html");
}

#[test]
fn manifest_entry_file_defaults_to_index_html() {
    let mut manifest = sample_manifest();
    manifest.entry = None;
    assert_eq!(manifest_entry_file(&manifest), "index.html");
}

#[test]
fn manifest_icon_file_uses_explicit_icon() {
    let manifest = sample_manifest();
    assert_eq!(manifest_icon_file(&manifest), "icon.svg");
}

#[test]
fn manifest_icon_file_defaults_to_icon_png() {
    let mut manifest = sample_manifest();
    manifest.icon = None;
    assert_eq!(manifest_icon_file(&manifest), "icon.png");
}
