use std::collections::BTreeMap;
use serde::Deserialize;
use specta::Type;
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, State, WebviewUrl};
use tauri::webview::NewWindowResponse;
use crate::{sandbox, AppsHostState};
use crate::runtime::{build_entry_src, emit_runtime_manager_runtimes_changed, inline_label_for, is_allowed_app_url, resolve_app, runtime_id_for, runtime_kind_for_app, should_use_incognito, SageAppRuntimeRecord};
use crate::storage::parse_data_store_id;
use crate::types::InstalledSageAppStorage;
use crate::utils::unix_timestamp_ms;

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

pub async fn create_inline_runtime(
    app: AppHandle,
    apps_state: State<'_, AppsHostState>,
    args: CreateInlineRuntimeArgs,
) -> Result<SageAppRuntimeRecord, String> {
    let base_path = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?;

    let resolved = resolve_app(&base_path, &args.app_id)?;
    let runtime_kind = runtime_kind_for_app(&resolved);
    let is_builtin_test_app = resolved.id().starts_with("__sage_test_");

    if !args.internal && !is_builtin_test_app {
        let baseline = apps_state.sandbox.baseline.lock().await.clone();
        let current_run = apps_state.sandbox.current_run.lock().await.clone();
        let effective =
            sandbox::state_view::build_effective_state(&baseline, current_run.as_ref());
        let gate = sandbox::evaluate_app_launch_gate(&resolved, &effective);

        if !gate.allowed {
            return Err(
                gate.message
                    .unwrap_or_else(|| "App launch blocked by sandbox policy".into()),
            );
        }
    }

    let is_incognito = should_use_incognito(&resolved);

    let host_window = app
        .get_window("main")
        .ok_or_else(|| "missing main window".to_string())?;

    let webview_label = inline_label_for(resolved.id(), runtime_kind);
    let runtime_id = runtime_id_for(resolved.id(), runtime_kind);
    let entry_src = build_entry_src(&resolved, args.path.clone(), args.query.clone());

    if let Some(existing) = host_window.get_webview(&webview_label) {
        return reuse_existing_inline_runtime(
            &apps_state,
            &existing,
            runtime_id,
            resolved.id().to_string(),
            resolved.name().to_string(),
            entry_src,
            webview_label,
            runtime_kind,
            args.visible,
            args.internal,
        )
            .await;
    }

    let origin_id_for_nav = resolved.origin_id().to_string();
    let runtime_kind_for_nav = runtime_kind;

    let mut builder = tauri::webview::WebviewBuilder::new(
        &webview_label,
        WebviewUrl::External(
            entry_src
                .parse()
                .map_err(|e| format!("invalid entry url: {e}"))?,
        ),
    )
        .on_navigation(move |url| {
            is_allowed_app_url(url, &origin_id_for_nav, runtime_kind_for_nav)
        })
        .on_new_window(move |_url, _features| NewWindowResponse::Deny);

    if is_incognito {
        builder = builder.incognito(true);
    } else {
        match resolved.storage() {
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            InstalledSageAppStorage::AppleDataStore { identifier_hex } => {
                let identifier = parse_data_store_id(identifier_hex)?;
                builder = builder.data_store_identifier(identifier);
            }

            #[cfg(target_os = "windows")]
            InstalledSageAppStorage::WindowsProfile { directory_name } => {
                builder = builder.data_directory(crate::storage::data_directory_for(directory_name));
            }

            _ => {}
        }
    }

    let debug = args.debug_layout;
    let (x, y, width, height) = if debug {
        debug_layout_for_app(resolved.id())
    } else {
        (0.0, 0.0, 1.0, 1.0)
    };

    host_window
        .add_child(
            builder,
            LogicalPosition::new(x, y),
            LogicalSize::new(width, height),
        )
        .map_err(|e| format!("failed to create child webview: {e}"))?;

    let now = unix_timestamp_ms();

    let record = SageAppRuntimeRecord {
        runtime_id: runtime_id.clone(),
        app_id: resolved.id().to_string(),
        app_name: resolved.name().to_string(),
        entry_src,
        webview_label: webview_label.clone(),
        host_window_label: "main".into(),
        runtime_kind,
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
        runtime_by_app_id.insert(resolved.id().to_string(), runtime_id);
    }

    if !args.visible {
        if let Some(webview) = host_window.get_webview(&webview_label) {
            let _ = webview.hide();
        }
    }

    emit_runtime_manager_runtimes_changed(&app, &apps_state).await;

    Ok(record)
}

async fn reuse_existing_inline_runtime(
    apps_state: &State<'_, AppsHostState>,
    webview: &tauri::Webview,
    runtime_id: String,
    app_id: String,
    app_name: String,
    entry_src: String,
    webview_label: String,
    runtime_kind: super::records::SageAppRuntimeKind,
    visible: bool,
    internal: bool,
) -> Result<SageAppRuntimeRecord, String> {
    let now = unix_timestamp_ms();

    if visible {
        webview
            .show()
            .map_err(|e| format!("failed to show existing child webview: {e}"))?;
    } else {
        webview
            .hide()
            .map_err(|e| format!("failed to hide existing child webview: {e}"))?;
    }

    let mut record = {
        let by_runtime_id = apps_state.runtime.by_runtime_id.lock().await;
        by_runtime_id.get(&runtime_id).cloned()
    }
        .unwrap_or_else(|| SageAppRuntimeRecord {
            runtime_id: runtime_id.clone(),
            app_id: app_id.clone(),
            app_name,
            entry_src,
            webview_label: webview_label.clone(),
            host_window_label: "main".into(),
            runtime_kind,
            mode: "inline".into(),
            state: "hidden".into(),
            started_at: now,
            last_active_at: now,
            visible: false,
            internal,
            active_batch_count: 0,
            active_socket_count: 0,
            in_flight_request_count: 0,
        });

    record.visible = visible;
    record.state = if visible {
        "running".into()
    } else {
        "hidden".into()
    };
    record.last_active_at = now;
    record.internal = internal;

    {
        let mut by_runtime_id = apps_state.runtime.by_runtime_id.lock().await;
        by_runtime_id.insert(runtime_id.clone(), record.clone());
    }

    {
        let mut runtime_by_app_id = apps_state.runtime.runtime_by_app_id.lock().await;
        runtime_by_app_id.insert(app_id, runtime_id);
    }

    Ok(record)
}

fn fallback_debug_slot(app_id: &str) -> usize {
    app_id
        .bytes()
        .fold(0usize, |acc, b| acc.wrapping_mul(31).wrapping_add(b as usize))
        % 12
}

fn debug_layout_for_app(app_id: &str) -> (f64, f64, f64, f64) {
    let slot = match app_id {
        "__sage_test_storage_isolation_persistent" => 0,
        "__sage_test_storage_isolation_incognito" => 1,
        "__sage_test_persistence_persistent" => 2,
        "__sage_test_persistence_incognito" => 3,
        "__sage_test_storage_clear_persistent" => 4,
        "__sage_test_network_allow_a" => 5,
        "__sage_test_network_allow_b" => 6,
        _ => fallback_debug_slot(app_id),
    };

    let cols = 3usize;
    let cell_w = 360.0;
    let cell_h = 100.0;
    let margin_x = 24.0;
    let margin_y = 24.0;
    let origin_x = 40.0;
    let origin_y = 40.0;

    let col = slot % cols;
    let row = slot / cols;

    let x = origin_x + col as f64 * (cell_w + margin_x);
    let y = origin_y + row as f64 * (cell_h + margin_y);

    (x, y, cell_w, cell_h)
}
