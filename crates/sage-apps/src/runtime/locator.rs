use tauri::{AppHandle, Manager};

const SAGE_WINDOW_LABEL: &str = "main";
const SAGE_WEBVIEW_LABEL: &str = "main";

pub(crate) fn get_sage_window(app: &AppHandle) -> Result<tauri::Window, String> {
    app
        .get_window(SAGE_WINDOW_LABEL)
        .ok_or_else(|| "missing sage window".to_string())
}

pub(crate) fn get_webview_in_sage_window(app: &AppHandle, webview_label: &str) -> Result<tauri::Webview, String> {
    get_sage_window(app)?.get_webview(webview_label)
        .ok_or_else(|| format!("missing '{}' webview", webview_label).to_string())
}

pub(crate) fn get_sage_webview(app: &AppHandle) -> Result<tauri::Webview, String> {
    get_webview_in_sage_window(app, SAGE_WEBVIEW_LABEL)
}
