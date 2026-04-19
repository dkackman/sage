use std::collections::BTreeMap;
#[cfg(target_os = "windows")]
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::webview::NewWindowResponse;
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, State, WebviewUrl};
use tokio::sync::Mutex;
use url::Url;

use crate::apps::builtin_apps::build_builtin_test_app;
use crate::apps::registry::read_installed_app_by_id;
use crate::apps::state::AppsHostState;
use crate::apps::types::InstalledSageApp;

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SageAppRuntimeRecord {
    pub runtime_id: String,
    pub app_id: String,
    pub app_name: String,
    pub entry_src: String,
    pub webview_label: String,
    pub host_window_label: String,
    pub mode: String,
    pub state: String,
    pub started_at: i64,
    pub last_active_at: i64,
    pub visible: bool,
    pub internal: bool,
    pub active_batch_count: u32,
    pub active_socket_count: u32,
    pub in_flight_request_count: u32,
}

#[derive(Default)]
pub struct AppRuntimeState {
    pub by_runtime_id: Mutex<BTreeMap<String, SageAppRuntimeRecord>>,
    pub runtime_by_app_id: Mutex<BTreeMap<String, String>>,
}

fn runtime_id_for(app_id: &str) -> String {
    format!("runtime-{app_id}")
}

fn inline_label_for(app_id: &str) -> String {
    format!("app-inline-{app_id}")
}

#[cfg(target_os = "windows")]
fn data_directory_for(app_id: &str) -> PathBuf {
    PathBuf::from("profiles").join(app_id)
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn data_store_id_for(app_id: &str) -> [u8; 16] {
    let bytes = app_id.as_bytes();
    let mut out = [0u8; 16];

    for (i, byte) in bytes.iter().enumerate() {
        out[i % 16] = out[i % 16].wrapping_add(*byte).wrapping_add(i as u8);
    }

    out
}

fn app_id_from_inline_label(label: &str) -> Option<&str> {
    label.strip_prefix("app-inline-")
}

fn is_allowed_app_url(url: &Url, app_id: &str) -> bool {
    url.scheme() == "sage-app" && url.host_str() == Some(app_id)
}

fn build_entry_src(
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

pub fn resolve_app(base_path: &std::path::Path, app_id: &str) -> Result<InstalledSageApp, String> {
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

fn should_use_incognito(app: &InstalledSageApp) -> bool {
    let has_persistent_storage = app
        .granted_permissions
        .capabilities
        .iter()
        .any(|cap| cap == "persistent_storage");

    if !has_persistent_storage {
        return true;
    }

    if app.permission_flags.storage_may_contain_secrets {
        return true;
    }

    false
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CreateInlineRuntimeArgs {
    pub app_id: String,
    pub visible: bool,
    pub internal: bool,
    pub debug_layout: bool,
    pub path: Option<String>,
    pub query: BTreeMap<String, String>,
}

#[tauri::command]
#[specta::specta]
pub async fn apps_create_inline_runtime(
    app: AppHandle,
    apps_state: State<'_, AppsHostState>,
    args: CreateInlineRuntimeArgs,
) -> Result<SageAppRuntimeRecord, String> {
    let base_path = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?;

    let installed = resolve_app(&base_path, &args.app_id)?;
    let is_builtin_test_app = installed.id.starts_with("__sage_test_");

    if !args.internal && !is_builtin_test_app {
        let baseline = apps_state.sandbox.baseline.lock().await.clone();
        let current_run = apps_state.sandbox.current_run.lock().await.clone();
        let effective =
            crate::apps::sandbox::state_view::build_effective_state(&baseline, current_run.as_ref());
        let gate = crate::apps::sandbox::evaluate_app_launch_gate(&installed, &effective);

        if !gate.allowed {
            return Err(
                gate.message
                    .unwrap_or_else(|| "App launch blocked by sandbox policy".into()),
            );
        }
    }

    let is_incognito = should_use_incognito(&installed);

    let host_window = app
        .get_window("main")
        .ok_or_else(|| "missing main window".to_string())?;

    let webview_label = inline_label_for(&installed.id);
    let runtime_id = runtime_id_for(&installed.id);
    let entry_src = build_entry_src(&installed, args.path.clone(), args.query.clone());

    if let Some(existing) = host_window.get_webview(&webview_label) {
        let _ = existing.close();
    }

    let app_id_for_nav = installed.id.clone();

    let mut builder = tauri::webview::WebviewBuilder::new(
        &webview_label,
        WebviewUrl::External(
            entry_src
                .parse()
                .map_err(|e| format!("invalid entry url: {e}"))?,
        ),
    )
        .on_navigation(move |url| is_allowed_app_url(url, &app_id_for_nav))
        .on_new_window(move |_url, _features| NewWindowResponse::Deny);

    if is_incognito {
        builder = builder.incognito(true);
    } else {
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        {
            builder = builder.data_store_identifier(data_store_id_for(&installed.id));
        }

        #[cfg(target_os = "windows")]
        {
            builder = builder.data_directory(data_directory_for(&installed.id));
        }
    }

    let debug = args.debug_layout;
    let x = if debug { 80.0 } else { 0.0 };
    let y = if debug { 80.0 } else { 0.0 };
    let width = if debug { 200.0 } else { 1.0 };
    let height = if debug { 200.0 } else { 1.0 };

    host_window
        .add_child(
            builder,
            LogicalPosition::new(x, y),
            LogicalSize::new(width, height),
        )
        .map_err(|e| format!("failed to create child webview: {e}"))?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("system clock error: {e}"))?
        .as_millis() as i64;

    let record = SageAppRuntimeRecord {
        runtime_id: runtime_id.clone(),
        app_id: installed.id.clone(),
        app_name: installed.name.clone(),
        entry_src,
        webview_label: webview_label.clone(),
        host_window_label: "main".into(),
        mode: "inline".into(),
        state: if args.visible {
            "running".into()
        } else {
            "hidden".into()
        },
        started_at: now,
        last_active_at: now,
        visible: args.visible,
        internal: args.internal,
        active_batch_count: 0,
        active_socket_count: 0,
        in_flight_request_count: 0,
    };

    {
        let mut by_runtime_id = apps_state.runtime.by_runtime_id.lock().await;
        by_runtime_id.insert(runtime_id.clone(), record.clone());
    }

    {
        let mut runtime_by_app_id = apps_state.runtime.runtime_by_app_id.lock().await;
        runtime_by_app_id.insert(installed.id.clone(), runtime_id);
    }

    if !args.visible {
        if let Some(webview) = host_window.get_webview(&webview_label) {
            let _ = webview.hide();
        }
    }

    Ok(record)
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

#[tauri::command]
#[specta::specta]
pub async fn apps_clear_runtime_browsing_data(
    app: AppHandle,
    app_id: String,
) -> Result<(), String> {
    let webview_label = inline_label_for(&app_id);

    if let Some(host_window) = app.get_window("main") {
        if let Some(existing) = host_window.get_webview(&webview_label) {
            let _ = existing.close();
        }
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        let target_id = data_store_id_for(&app_id);
        let existing_ids = app
            .fetch_data_store_identifiers()
            .await
            .map_err(|e| format!("failed to fetch data store identifiers: {e}"))?;

        if existing_ids.iter().any(|id| *id == target_id) {
            app.remove_data_store(target_id)
                .await
                .map_err(|e| format!("failed to remove data store for {app_id}: {e}"))?;
        }
    }

    #[cfg(target_os = "windows")]
    {
        let app_data_dir = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("failed to resolve app data dir: {e}"))?;

        let profile_dir = app_data_dir.join(data_directory_for(&app_id));

        match std::fs::remove_dir_all(&profile_dir) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                return Err(format!(
                    "failed to remove profile dir {}: {err}",
                    profile_dir.display()
                ));
            }
        }
    }

    Ok(())
}

pub(crate) async fn close_runtime_internal(
    app: &AppHandle,
    apps_state: &State<'_, AppsHostState>,
    app_id: &str,
) -> Result<(), String> {
    let runtime_id = {
        let runtime_by_app_id = apps_state.runtime.runtime_by_app_id.lock().await;
        runtime_by_app_id.get(app_id).cloned()
    };

    let Some(runtime_id) = runtime_id else {
        return Ok(());
    };

    let record = {
        let by_runtime_id = apps_state.runtime.by_runtime_id.lock().await;
        by_runtime_id.get(&runtime_id).cloned()
    };

    let Some(record) = record else {
        let mut runtime_by_app_id = apps_state.runtime.runtime_by_app_id.lock().await;
        runtime_by_app_id.remove(app_id);
        return Ok(());
    };

    if let Some(host_window) = app.get_window("main") {
        if let Some(webview) = host_window.get_webview(&record.webview_label) {
            let _ = webview.close();
        }
    }

    {
        let mut by_runtime_id = apps_state.runtime.by_runtime_id.lock().await;
        by_runtime_id.remove(&runtime_id);
    }

    {
        let mut runtime_by_app_id = apps_state.runtime.runtime_by_app_id.lock().await;
        runtime_by_app_id.remove(app_id);
    }

    Ok(())
}

pub(crate) async fn clear_runtime_browsing_data_internal(
    app: &AppHandle,
    apps_state: &State<'_, AppsHostState>,
    app_id: &str,
) -> Result<(), String> {
    let _ = close_runtime_internal(app, apps_state, app_id).await;
    apps_clear_runtime_browsing_data(app.clone(), app_id.to_string()).await
}

pub(crate) async fn start_internal_runtime_for_sandbox(
    app: &AppHandle,
    apps_state: &State<'_, AppsHostState>,
    app_id: &str,
    visible: bool,
    path: Option<String>,
    query: BTreeMap<String, String>,
) -> Result<(), String> {
    let args = CreateInlineRuntimeArgs {
        app_id: app_id.to_string(),
        visible,
        internal: true,
        debug_layout: false,
        path,
        query,
    };

    apps_create_inline_runtime(app.clone(), apps_state.clone(), args)
        .await
        .map(|_| ())
}
