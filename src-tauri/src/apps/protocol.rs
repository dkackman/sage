use std::{fs, path::{Path, PathBuf}};

use anyhow::{anyhow, Context, Result as AnyResult};
use tauri::http::{Response, StatusCode};

use crate::apps::{
    bootstrap::build_sage_bootstrap,
    csp::build_app_csp,
    install::{app_install_dir, read_installed_app_by_id},
};

fn resolve_protocol_file(base_path: &Path, app_id: &str, request_path: &str) -> AnyResult<PathBuf> {
    let install_dir = app_install_dir(base_path, app_id);

    if request_path == "/__sage_bootstrap__.js" {
        return Ok(install_dir.join("__virtual_sage_bootstrap__.js"));
    }

    let path = if request_path.is_empty() || request_path == "/" || request_path == "/index.html" {
        install_dir.join("dist").join("index.html")
    } else if request_path == "/icon.png" {
        install_dir.join("icon.png")
    } else {
        let trimmed = request_path.trim_start_matches('/');

        if trimmed.contains("..") {
            return Err(anyhow!("invalid app path"));
        }

        install_dir.join("dist").join(trimmed)
    };

    let canonical_install_dir = install_dir.canonicalize().with_context(|| {
        format!(
            "failed to canonicalize install dir {}",
            install_dir.display()
        )
    })?;

    let canonical_path = path.canonicalize().with_context(|| {
        format!(
            "failed to canonicalize requested path {}",
            path.display()
        )
    })?;

    if !canonical_path.starts_with(&canonical_install_dir) {
        return Err(anyhow!("requested path escapes app directory"));
    }

    if !canonical_path.is_file() {
        return Err(anyhow!("requested file does not exist"));
    }

    Ok(canonical_path)
}

fn inject_bootstrap_into_index_html(html: &str) -> String {
    let bootstrap_tag = r#"<script src="/__sage_bootstrap__.js"></script>"#;

    if let Some(idx) = html.find("<head>") {
        let insert_at = idx + "<head>".len();
        let mut out = String::with_capacity(html.len() + bootstrap_tag.len());
        out.push_str(&html[..insert_at]);
        out.push_str(bootstrap_tag);
        out.push_str(&html[insert_at..]);
        out
    } else {
        format!("{bootstrap_tag}{html}")
    }
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

    let file_path = resolve_protocol_file(base_path, app_id, request_path)?;
    let app = read_installed_app_by_id(base_path, app_id)?;

    if request_path == "/__sage_bootstrap__.js" {
        let js = build_sage_bootstrap(&app);

        return Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/javascript; charset=utf-8")
            .body(js.into_bytes())
            .map_err(|err| anyhow!("failed to build protocol response: {err}"));
    }

    if request_path.is_empty() || request_path == "/" || request_path == "/index.html" {
        let html = fs::read_to_string(&file_path)
            .with_context(|| format!("failed to read {}", file_path.display()))?;

        let injected = inject_bootstrap_into_index_html(&html);
        let csp = build_app_csp(&app);

        return Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/html; charset=utf-8")
            .header("Content-Security-Policy", csp)
            .body(injected.into_bytes())
            .map_err(|err| anyhow!("failed to build protocol response: {err}"));
    }

    let bytes =
        fs::read(&file_path).with_context(|| format!("failed to read {}", file_path.display()))?;

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
