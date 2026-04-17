use std::{fs, path::Path};

use anyhow::{anyhow, Result as AnyResult};
use tauri::http::{Response, StatusCode};

use crate::apps::{
    csp::build_app_csp,
    snapshot::read_snapshot_file,
};
use crate::apps::registry::read_installed_app_by_id;

const CLEAR_STUB_PATH: &str = "/__sage_clear__.html";

fn build_clear_stub_html() -> String {
    r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <title>Sage Clear</title>
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <style>
      html, body {
        margin: 0;
        padding: 0;
        background: #0a0a0a;
        color: #cfcfcf;
        font: 12px system-ui, sans-serif;
      }
      body {
        display: flex;
        align-items: center;
        justify-content: center;
        min-height: 100vh;
      }
    </style>
  </head>
  <body>
    Clearing app browsing data…
  </body>
</html>
"#.to_string()
}

fn build_clear_stub_csp() -> &'static str {
    "default-src 'none'; \
     script-src 'none'; \
     style-src 'unsafe-inline'; \
     img-src 'none'; \
     font-src 'none'; \
     media-src 'none'; \
     connect-src 'none'; \
     frame-src 'none'; \
     child-src 'none'; \
     worker-src 'none'; \
     object-src 'none'; \
     base-uri 'none'; \
     form-action 'none';"
}

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

    if request_path == CLEAR_STUB_PATH {
        let html = build_clear_stub_html();

        return Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/html; charset=utf-8")
            .header("Content-Security-Policy", build_clear_stub_csp())
            .header("X-Content-Type-Options", "nosniff")
            .body(html.into_bytes())
            .map_err(|err| anyhow!("failed to build clear stub response: {err}"));
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
        .header("X-Content-Type-Options", "nosniff")
        .body(bytes)
        .map_err(|err| anyhow!("failed to build protocol response: {err}"))
}
