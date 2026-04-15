use std::{fs, path::Path};

use anyhow::{anyhow, Result as AnyResult};
use tauri::http::{Response, StatusCode};

use crate::apps::{
    csp::build_app_csp,
    snapshot::read_snapshot_file,
};
use crate::apps::registry::read_installed_app_by_id;

pub fn handle_app_protocol_request(
    base_path: &Path,
    request: &tauri::http::Request<Vec<u8>>,
) -> AnyResult<Response<Vec<u8>>> {
    let uri = request.uri();

    let app_id = uri
        .host()
        .ok_or_else(|| anyhow!("missing app id in sage-app URL"))?;

    let request_path = uri.path();
    let app = read_installed_app_by_id(base_path, app_id)?;

    let snapshot_dir = Path::new(&app.active_snapshot.snapshot_dir);
    let file_path = read_snapshot_file(snapshot_dir, request_path)?;

    if request_path.is_empty() || request_path == "/" || request_path == "/index.html" {
        let html = fs::read_to_string(&file_path)?;
        let csp = build_app_csp(&app);

        return Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/html; charset=utf-8")
            .header("Content-Security-Policy", csp)
            .body(html.into_bytes())
            .map_err(|err| anyhow!("failed to build protocol response: {err}"));
    }

    let bytes = fs::read(&file_path)?;
    let mime = mime_guess::from_path(&file_path)
        .first_or_octet_stream()
        .essence_str()
        .to_string();

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", mime)
        .body(bytes)
        .map_err(|err| anyhow!("failed to build protocol response: {err}"))
}
