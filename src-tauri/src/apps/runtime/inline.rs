use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use specta::Type;
use tauri::webview::NewWindowResponse;
use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, State, WebviewUrl,
};

#[cfg(target_os = "windows")]
use std::path::PathBuf;

use crate::apps::sandbox;
use crate::apps::state::AppsHostState;
use crate::apps::types::InstalledSageAppStorage;

use super::records::{inline_label_for, runtime_id_for, SageAppRuntimeRecord};
use super::resolve::{build_entry_src, is_allowed_app_url, resolve_app, should_use_incognito};

#[cfg(target_os = "windows")]
fn data_directory_for(directory_name: &str) -> PathBuf {
    PathBuf::from("profiles").join(directory_name)
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn parse_data_store_id(identifier_hex: &str) -> Result<[u8; 16], String> {
    let bytes = hex::decode(identifier_hex)
        .map_err(|err| format!("invalid data store identifier hex: {err}"))?;

    if bytes.len() != 16 {
        return Err(format!(
            "invalid data store identifier length {}, expected 16 bytes",
            bytes.len()
        ));
    }

    let mut out = [0_u8; 16];
    out.copy_from_slice(&bytes);
    Ok(out)
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
            sandbox::state_view::build_effective_state(&baseline, current_run.as_ref());
        let gate = sandbox::evaluate_app_launch_gate(&installed, &effective);

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
        match &installed.storage {
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            InstalledSageAppStorage::AppleDataStore { identifier_hex } => {
                let identifier = parse_data_store_id(identifier_hex)?;
                builder = builder.data_store_identifier(identifier);
            }

            #[cfg(target_os = "windows")]
            InstalledSageAppStorage::WindowsProfile { directory_name } => {
                builder = builder.data_directory(data_directory_for(directory_name));
            }

            _ => {}
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
