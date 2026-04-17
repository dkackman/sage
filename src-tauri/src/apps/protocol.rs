use std::{fs, path::Path};

use anyhow::{anyhow, Result as AnyResult};
use tauri::{
    http::{Response, StatusCode},
    AppHandle, Manager,
};

use crate::apps::{
    csp::build_app_csp,
    sandbox::{store_isolation_probe_result, SandboxIsolationProbeResult, SandboxProbeStore},
    snapshot::read_snapshot_file,
};
use crate::apps::registry::read_installed_app_by_id;

const SAGE_PROBE_LOCAL_STORAGE_KEY: &str = "sage_probe_local_storage";
const SAGE_PROBE_COOKIE_KEY: &str = "sage_probe_cookie";
const SAGE_PROBE_INDEXED_DB_NAME: &str = "sage_probe_db";
const SAGE_PROBE_INDEXED_DB_STORE: &str = "probe_store";
const SAGE_PROBE_INDEXED_DB_KEY: &str = "sage_probe_key";

fn build_blank_internal_response() -> AnyResult<Response<Vec<u8>>> {
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/html; charset=utf-8")
        .header("Cache-Control", "no-store")
        .header("X-Content-Type-Options", "nosniff")
        .body(
            b"<!doctype html><html><head><meta charset=\"utf-8\"></head><body></body></html>"
                .to_vec(),
        )
        .map_err(|err| anyhow!("failed to build blank internal response: {err}"))
}

fn build_sandbox_isolation_probe_response(
    run_id: &str,
    mode: &str,
    persistent_storage: bool,
) -> AnyResult<Response<Vec<u8>>> {
    let html = format!(
        r#"<!doctype html>
<html>
  <head>
    <meta charset="utf-8">
    <title>Sandbox Isolation Probe</title>
  </head>
  <body>
    <script>
      const RUN_ID = {run_id:?};
      const MODE = {mode:?};
      const PERSISTENT_STORAGE = {persistent_storage};

      const LOCAL_STORAGE_KEY = {local_storage_key:?};
      const COOKIE_KEY = {cookie_key:?};
      const DB_NAME = {db_name:?};
      const STORE_NAME = {store_name:?};
      const DB_KEY = {db_key:?};

      async function readIndexedDbProbe() {{
        try {{
          return await new Promise((resolve) => {{
            const open = indexedDB.open(DB_NAME);
            open.onerror = () => resolve(false);
            open.onupgradeneeded = () => {{
              try {{
                const db = open.result;
                if (!db.objectStoreNames.contains(STORE_NAME)) {{
                  db.createObjectStore(STORE_NAME);
                }}
              }} catch {{
                // ignore
              }}
            }};
            open.onsuccess = () => {{
              try {{
                const db = open.result;
                if (!db.objectStoreNames.contains(STORE_NAME)) {{
                  resolve(false);
                  db.close();
                  return;
                }}

                const tx = db.transaction(STORE_NAME, 'readonly');
                const store = tx.objectStore(STORE_NAME);
                const req = store.get(DB_KEY);

                req.onerror = () => {{
                  resolve(false);
                  db.close();
                }};

                req.onsuccess = () => {{
                  resolve(typeof req.result === 'string' && req.result.length > 0);
                  db.close();
                }};
              }} catch {{
                resolve(false);
              }}
            }};
          }});
        }} catch {{
          return false;
        }}
      }}

      async function report(result) {{
        await fetch('sage-app://__sandbox/report', {{
          method: 'POST',
          headers: {{
            'Content-Type': 'application/json',
          }},
          body: JSON.stringify(result),
        }});
      }}

      async function main() {{
        let localStorageVisible = false;
        let cookieVisible = false;
        let indexedDbVisible = false;
        let error = null;

        try {{
          try {{
            const value = localStorage.getItem(LOCAL_STORAGE_KEY);
            localStorageVisible = typeof value === 'string' && value.length > 0;
          }} catch {{
            localStorageVisible = false;
          }}

          try {{
            cookieVisible = document.cookie
              .split(';')
              .map((part) => part.trim())
              .some((part) => part.startsWith(COOKIE_KEY + '='));
          }} catch {{
            cookieVisible = false;
          }}

          indexedDbVisible = await readIndexedDbProbe();
        }} catch (err) {{
          error = err instanceof Error ? err.message : String(err);
        }}

        await report({{
          runId: RUN_ID,
          mode: MODE,
          persistentStorage: PERSISTENT_STORAGE,
          localStorageVisible,
          cookieVisible,
          indexedDbVisible,
          error,
        }});
      }}

      void main();
    </script>
  </body>
</html>
"#,
        run_id = run_id,
        mode = mode,
        persistent_storage = if persistent_storage { "true" } else { "false" },
        local_storage_key = SAGE_PROBE_LOCAL_STORAGE_KEY,
        cookie_key = SAGE_PROBE_COOKIE_KEY,
        db_name = SAGE_PROBE_INDEXED_DB_NAME,
        store_name = SAGE_PROBE_INDEXED_DB_STORE,
        db_key = SAGE_PROBE_INDEXED_DB_KEY,
    );

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/html; charset=utf-8")
        .header("Cache-Control", "no-store")
        .header("X-Content-Type-Options", "nosniff")
        .body(html.into_bytes())
        .map_err(|err| anyhow!("failed to build sandbox isolation probe response: {err}"))
}

fn build_sandbox_ok_response() -> AnyResult<Response<Vec<u8>>> {
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json; charset=utf-8")
        .header("Cache-Control", "no-store")
        .header("X-Content-Type-Options", "nosniff")
        .body(br#"{"ok":true}"#.to_vec())
        .map_err(|err| anyhow!("failed to build sandbox ok response: {err}"))
}

fn parse_query_param(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let mut parts = pair.splitn(2, '=');
        let k = parts.next()?;
        let v = parts.next().unwrap_or_default();
        if k == key {
            Some(v.to_string())
        } else {
            None
        }
    })
}

fn handle_sandbox_protocol_request(
    app_handle: &AppHandle,
    request: &tauri::http::Request<Vec<u8>>,
) -> AnyResult<Response<Vec<u8>>> {
    let uri = request.uri();
    let path = uri.path();

    match (request.method().as_str(), path) {
        ("GET", "/isolation-check") => {
            let query = uri.query().unwrap_or_default();
            let run_id = parse_query_param(query, "runId")
                .ok_or_else(|| anyhow!("missing runId in sandbox isolation probe request"))?;
            let mode = parse_query_param(query, "mode")
                .ok_or_else(|| anyhow!("missing mode in sandbox isolation probe request"))?;
            let persistent_storage = parse_query_param(query, "persistentStorage")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);

            build_sandbox_isolation_probe_response(&run_id, &mode, persistent_storage)
        }

        ("POST", "/report") => {
            let result: SandboxIsolationProbeResult = serde_json::from_slice(request.body())
                .map_err(|err| anyhow!("failed to parse sandbox probe result body: {err}"))?;

            let store = app_handle.state::<SandboxProbeStore>();
            store_isolation_probe_result(&store, result);

            build_sandbox_ok_response()
        }

        _ => Err(anyhow!("unknown sandbox protocol path: {}", path)),
    }
}

pub fn handle_app_protocol_request(
    base_path: &Path,
    app_handle: &AppHandle,
    request: &tauri::http::Request<Vec<u8>>,
) -> AnyResult<Response<Vec<u8>>> {
    let uri = request.uri();

    let host = uri
        .host()
        .ok_or_else(|| anyhow!("missing host in sage-app URL"))?;

    if host == "__sandbox" {
        return handle_sandbox_protocol_request(app_handle, request);
    }

    let app = read_installed_app_by_id(base_path, host)?;
    let request_path = uri.path();

    if request_path == "/__sage/blank" {
        return build_blank_internal_response();
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
