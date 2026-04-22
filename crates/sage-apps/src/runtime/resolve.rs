use std::collections::BTreeMap;
use std::path::Path;

use tauri::{AppHandle, Manager};
use url::Url;

use crate::lifecycle::read_installed_app_by_id;
use crate::sandbox::build_builtin_test_app;
use crate::types::SageApp;

use super::records::{inline_label_for, SageAppRuntimeKind};

fn app_id_from_inline_label(label: &str) -> Option<(SageAppRuntimeKind, &str)> {
    if let Some(app_id) = label.strip_prefix("app-inline-") {
        return Some((SageAppRuntimeKind::User, app_id));
    }

    if let Some(app_id) = label.strip_prefix("system-app-inline-") {
        return Some((SageAppRuntimeKind::System, app_id));
    }

    None
}

pub fn runtime_kind_for_app(app: &SageApp) -> SageAppRuntimeKind {
    match app {
        SageApp::User(_) => SageAppRuntimeKind::User,
        SageApp::System(_) => SageAppRuntimeKind::System,
    }
}

pub fn protocol_scheme_for_runtime_kind(runtime_kind: SageAppRuntimeKind) -> &'static str {
    match runtime_kind {
        SageAppRuntimeKind::User => "sage-app",
        SageAppRuntimeKind::System => "sage-system-app",
    }
}

pub fn is_allowed_app_url(
    url: &Url,
    origin_id: &str,
    runtime_kind: SageAppRuntimeKind,
) -> bool {
    url.scheme() == protocol_scheme_for_runtime_kind(runtime_kind)
        && url.host_str() == Some(origin_id)
}

pub fn build_entry_src(
    app: &SageApp,
    path: Option<String>,
    query: BTreeMap<String, String>,
) -> String {
    let runtime_kind = runtime_kind_for_app(app);
    let scheme = protocol_scheme_for_runtime_kind(runtime_kind);
    let entry_path = path.unwrap_or_else(|| format!("/{}", app.entry_file()));

    let mut url = Url::parse(&format!("{scheme}://{}{}", app.origin_id(), entry_path))
        .expect("failed to build app entry URL");

    for (key, value) in query {
        url.query_pairs_mut().append_pair(&key, &value);
    }

    url.to_string()
}

pub fn resolve_app(base_path: &Path, app_id: &str) -> Result<SageApp, String> {
    match read_installed_app_by_id(base_path, app_id) {
        Ok(app) => Ok(SageApp::User(app)),
        Err(installed_err) => build_builtin_test_app(app_id)
            .map_err(|builtin_err| {
                format!(
                    "failed to resolve app {app_id}: installed lookup error: {installed_err}; builtin lookup error: {builtin_err}"
                )
            })?
            .ok_or_else(|| format!("failed to read app {app_id}: {installed_err}")),
    }
}

pub fn should_use_incognito(app: &SageApp) -> bool {
    let has_persistent_storage = app
        .granted_permissions()
        .capabilities
        .iter()
        .any(|cap| cap == "persistent_storage");

    if !has_persistent_storage {
        return true;
    }

    if app.capability_flags().storage_may_contain_secrets {
        return true;
    }

    false
}

pub(crate) fn assert_bridge_origin(
    app: AppHandle,
    source_label: String,
) -> Result<(String, SageAppRuntimeKind), String> {
    let (runtime_kind, app_id) = app_id_from_inline_label(&source_label)
        .ok_or_else(|| format!("invalid app runtime label: {source_label}"))?;

    let app_id = app_id.to_string();

    let base_path = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?;

    let resolved = resolve_app(&base_path, &app_id)?;
    let expected_runtime_kind = runtime_kind_for_app(&resolved);

    if runtime_kind != expected_runtime_kind {
        return Err(format!(
            "bridge denied for {source_label}: runtime kind mismatch (label={runtime_kind:?}, app={expected_runtime_kind:?})"
        ));
    }

    let host_window = app
        .get_window("main")
        .ok_or_else(|| "missing main window".to_string())?;

    let webview = host_window
        .get_webview(&source_label)
        .ok_or_else(|| format!("missing webview for label: {source_label}"))?;

    let current_url = webview
        .url()
        .map_err(|e| format!("failed to read current webview url: {e}"))?;

    if !is_allowed_app_url(&current_url, resolved.origin_id(), runtime_kind) {
        return Err(format!(
            "bridge denied for {source_label}: current url {} is outside {}://{}/...",
            current_url,
            protocol_scheme_for_runtime_kind(runtime_kind),
            resolved.origin_id()
        ));
    }

    Ok((app_id, runtime_kind))
}

pub fn webview_label_for_app(app: &SageApp) -> String {
    inline_label_for(app.id(), runtime_kind_for_app(app))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use crate::types::{
        InstalledSageAppStorage, SageApp, SageAppCapabilityFlags, SageAppCommon,
        SageAppManifestFile, SageAppPackageManifest, SageAppSnapshot,
        SageGrantedNetworkPermissions, SageGrantedPermissions, SageRequestedPermissions,
        SystemAppPresentation, SystemSageApp,
    };

    fn sample_app(
        origin_id: &str,
        capabilities: Vec<&str>,
        storage_may_contain_secrets: bool,
    ) -> SageApp {
        SageApp::System(SystemSageApp {
            common: SageAppCommon {
                id: "url-abc123".into(),
                origin_id: origin_id.into(),
                name: "Test App".into(),
                version: "1.0.0".into(),
                app_dir: "/tmp/app".into(),
                entry_file: "index.html".into(),
                icon_file: "icon.png".into(),
                requested_permissions: SageRequestedPermissions::default(),
                granted_permissions: SageGrantedPermissions {
                    capabilities: capabilities.into_iter().map(|s| s.to_string()).collect(),
                    network: SageGrantedNetworkPermissions { whitelist: vec![] },
                },
                capability_flags: SageAppCapabilityFlags {
                    has_secret_access: false,
                    has_external_access: false,
                    storage_may_contain_secrets,
                    isolated: false,
                },
                storage: InstalledSageAppStorage::Unmanaged,
                active_snapshot: SageAppSnapshot {
                    manifest_hash: "hash".into(),
                    snapshot_dir: "/tmp/app".into(),
                    total_bytes: 1,
                    manifest: SageAppPackageManifest {
                        name: "Test App".into(),
                        version: "1.0.0".into(),
                        permissions: SageRequestedPermissions::default(),
                        files: vec![SageAppManifestFile {
                            path: "index.html".into(),
                            sha256: "a".repeat(64),
                            size: 1,
                        }],
                        entry: Some("index.html".into()),
                        icon: Some("icon.png".into()),
                        author: None,
                        donation: None,
                    },
                },
            },
            presentation: SystemAppPresentation::Taskbar,
        })
    }

    #[test]
    fn allowed_user_app_url_matches_origin_id() {
        let url = Url::parse("sage-app://origin-1/index.html").unwrap();
        assert!(is_allowed_app_url(&url, "origin-1", SageAppRuntimeKind::User));
        assert!(!is_allowed_app_url(&url, "origin-2", SageAppRuntimeKind::User));
    }

    #[test]
    fn allowed_system_app_url_matches_origin_id() {
        let url = Url::parse("sage-system-app://origin-1/index.html").unwrap();
        assert!(is_allowed_app_url(&url, "origin-1", SageAppRuntimeKind::System));
        assert!(!is_allowed_app_url(&url, "origin-2", SageAppRuntimeKind::System));
    }

    #[test]
    fn build_entry_src_uses_system_scheme_for_system_apps() {
        let app = sample_app("origin-1", vec![], false);
        let url = build_entry_src(&app, None, BTreeMap::new());
        assert_eq!(url, "sage-system-app://origin-1/index.html");
    }

    #[test]
    fn build_entry_src_supports_custom_path_and_query() {
        let app = sample_app("origin-1", vec![], false);
        let mut query = BTreeMap::new();
        query.insert("a".into(), "1".into());
        query.insert("b".into(), "hello".into());

        let url = build_entry_src(&app, Some("/nested/page.html".into()), query);

        assert!(url.starts_with("sage-system-app://origin-1/nested/page.html?"));
        assert!(url.contains("a=1"));
        assert!(url.contains("b=hello"));
    }

    #[test]
    fn should_use_incognito_without_persistent_storage() {
        let app = sample_app("origin-1", vec![], false);
        assert!(should_use_incognito(&app));
    }

    #[test]
    fn should_use_incognito_when_storage_is_tainted() {
        let app = sample_app("origin-1", vec!["persistent_storage"], true);
        assert!(should_use_incognito(&app));
    }

    #[test]
    fn should_not_use_incognito_when_persistent_storage_is_granted_and_clean() {
        let app = sample_app("origin-1", vec!["persistent_storage"], false);
        assert!(!should_use_incognito(&app));
    }

    #[test]
    fn webview_label_for_system_app_has_expected_shape() {
        let app = sample_app("origin-1", vec![], false);
        assert_eq!(webview_label_for_app(&app), "system-app-inline-url-abc123");
    }
}
