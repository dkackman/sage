use std::collections::BTreeMap;
use std::path::Path;

use tauri::{AppHandle, Manager};
use url::Url;

use crate::apps::lifecycle::read_installed_app_by_id;
use crate::apps::sandbox::build_builtin_test_app;
use crate::apps::types::InstalledSageApp;

use super::records::inline_label_for;

fn app_id_from_inline_label(label: &str) -> Option<&str> {
    label.strip_prefix("app-inline-")
}

pub fn is_allowed_app_url(url: &Url, app_id: &str) -> bool {
    url.scheme() == "sage-app" && url.host_str() == Some(app_id)
}

pub fn build_entry_src(
    app: &InstalledSageApp,
    path: Option<String>,
    query: BTreeMap<String, String>,
) -> String {
    let entry_path = path.unwrap_or_else(|| format!("/{}", app.entry_file));
    let mut url = Url::parse(&format!("sage-app://{}{}", app.id, entry_path))
        .expect("failed to build sage-app entry URL");

    for (key, value) in query {
        url.query_pairs_mut().append_pair(&key, &value);
    }

    url.to_string()
}

pub fn resolve_app(
    base_path: &Path,
    app_id: &str,
) -> Result<InstalledSageApp, String> {
    match read_installed_app_by_id(base_path, app_id) {
        Ok(app) => Ok(app),
        Err(installed_err) => build_builtin_test_app(app_id)
            .map_err(|builtin_err| {
                format!(
                    "failed to resolve app {app_id}: installed lookup error: {installed_err}; builtin lookup error: {builtin_err}"
                )
            })?
            .ok_or_else(|| format!("failed to read app {app_id}: {installed_err}")),
    }
}

pub fn should_use_incognito(app: &InstalledSageApp) -> bool {
    let has_persistent_storage = app
        .granted_permissions
        .capabilities
        .iter()
        .any(|cap| cap == "persistent_storage");

    if !has_persistent_storage {
        return true;
    }

    if app.capability_flags.storage_may_contain_secrets {
        return true;
    }

    false
}

#[tauri::command]
#[specta::specta]
pub fn apps_assert_bridge_origin(
    app: AppHandle,
    source_label: String,
) -> Result<String, String> {
    let app_id = app_id_from_inline_label(&source_label)
        .ok_or_else(|| format!("invalid app runtime label: {source_label}"))?
        .to_string();

    let host_window = app
        .get_window("main")
        .ok_or_else(|| "missing main window".to_string())?;

    let webview = host_window
        .get_webview(&source_label)
        .ok_or_else(|| format!("missing webview for label: {source_label}"))?;

    let current_url = webview
        .url()
        .map_err(|e| format!("failed to read current webview url: {e}"))?;

    if !is_allowed_app_url(&current_url, &app_id) {
        return Err(format!(
            "bridge denied for {source_label}: current url {} is outside sage-app://{app_id}/...",
            current_url
        ));
    }

    Ok(app_id)
}

pub fn webview_label_for_app(app_id: &str) -> String {
    inline_label_for(app_id)
}
