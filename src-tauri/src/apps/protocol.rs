use std::{fs, path::Path};

use anyhow::{anyhow, Result as AnyResult};
use tauri::http::{Response, StatusCode};

use crate::apps::{
    builtin_apps::{build_builtin_test_app, builtin_test_app_spec, builtin_test_app_dir},
    csp::build_app_csp,
    registry::read_installed_app_by_id,
    snapshot::read_snapshot_file,
};

fn build_clear_internal_response() -> AnyResult<Response<Vec<u8>>> {
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/html; charset=utf-8")
        .header("Cache-Control", "no-store")
        .header("Clear-Site-Data", "*")
        .header("X-Content-Type-Options", "nosniff")
        .body(
            b"<!doctype html><html><head><meta charset=\"utf-8\"></head><body></body></html>"
                .to_vec(),
        )
        .map_err(|err| anyhow!("failed to build clear internal response: {err}"))
}

fn handle_builtin_test_app_request(
    app_id: &str,
    request: &tauri::http::Request<Vec<u8>>,
) -> AnyResult<Response<Vec<u8>>> {
    let app = build_builtin_test_app(app_id)?
        .ok_or_else(|| anyhow!("unknown builtin test app {}", app_id))?;

    let request_path = request.uri().path();

    if request_path == "/__sage/clear" {
        return build_clear_internal_response();
    }

    let app_dir = builtin_test_app_dir(app_id)?
        .ok_or_else(|| anyhow!("missing builtin test app dir for {}", app_id))?;

    let file_path = read_snapshot_file(&app_dir, request_path)?;
    let csp = build_app_csp(&app);

    if request_path.is_empty() || request_path == "/" || request_path == "/index.html" {
        let html = fs::read_to_string(&file_path)?;

        return Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/html; charset=utf-8")
            .header("Cache-Control", "no-store")
            .header("Content-Security-Policy", &csp)
            .header("X-Content-Type-Options", "nosniff")
            .body(html.into_bytes())
            .map_err(|err| anyhow!("failed to build builtin test app HTML response: {err}"));
    }

    let bytes = fs::read(&file_path)?;
    let mime = mime_guess::from_path(&file_path)
        .first_or_octet_stream()
        .essence_str()
        .to_string();

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", mime)
        .header("Cache-Control", "no-store")
        .header("Content-Security-Policy", csp)
        .header("X-Content-Type-Options", "nosniff")
        .body(bytes)
        .map_err(|err| anyhow!("failed to build builtin test app response: {err}"))
}

pub fn handle_app_protocol_request(
    base_path: &Path,
    request: &tauri::http::Request<Vec<u8>>,
) -> AnyResult<Response<Vec<u8>>> {
    let uri = request.uri();

    let host = uri
        .host()
        .ok_or_else(|| anyhow!("missing host in sage-app URL"))?;

    if builtin_test_app_spec(host).is_some() {
        return handle_builtin_test_app_request(host, request);
    }

    let app = read_installed_app_by_id(base_path, host)?;
    let request_path = uri.path();

    if request_path == "/__sage/clear" {
        return build_clear_internal_response();
    }

    let snapshot_dir = Path::new(&app.active_snapshot.snapshot_dir);
    let file_path = read_snapshot_file(snapshot_dir, request_path)?;

    let csp = build_app_csp(&app);

    if request_path.is_empty() || request_path == "/" || request_path == "/index.html" {
        let html = fs::read_to_string(&file_path)?;

        return Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/html; charset=utf-8")
            .header("Content-Security-Policy", &csp)
            .header("Clear-Site-Data", "*")
            .header("X-Content-Type-Options", "nosniff")
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
        .header("Content-Security-Policy", csp)
        .header("Clear-Site-Data", "*")
        .header("X-Content-Type-Options", "nosniff")
        .body(bytes)
        .map_err(|err| anyhow!("failed to build protocol response: {err}"))
}
