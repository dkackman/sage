use std::{
    fs,
    io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result as AnyResult, anyhow};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{State, command};
use zip::ZipArchive;
use tauri::http::{Response, StatusCode};

use crate::{app_state::AppState, error::Result};

const INSTALLED_METADATA_FILE: &str = ".sage-installed.json";

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SageAppPermissions {
    pub network: bool,
    #[serde(rename = "persistentStorage", alias = "persistent_storage")]
    pub persistent_storage: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SageAppPackageManifest {
    pub name: String,
    pub version: String,
    pub permissions: SageAppPermissions,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct InstalledSageApp {
    pub id: String,
    pub name: String,
    pub version: String,

    #[serde(rename = "installDir", alias = "install_dir")]
    pub install_dir: String,

    #[serde(rename = "entryFile", alias = "entry_file")]
    pub entry_file: String,

    #[serde(rename = "iconFile", alias = "icon_file")]
    pub icon_file: String,

    pub permissions: SageAppPermissions,
}

fn apps_root(base_path: &Path) -> PathBuf {
    base_path.join("apps")
}

fn current_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_millis()
}

fn slugify_name(name: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;

    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }

    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "app".to_string()
    } else {
        out
    }
}

fn generate_app_id(name: &str) -> String {
    format!("{}-{}", slugify_name(name), current_millis())
}

fn should_ignore_root_entry(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
        return false;
    };

    name == "__MACOSX" || name == ".DS_Store"
}

fn copy_dir_all(src: &Path, dst: &Path) -> AnyResult<()> {
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let from = entry.path();
        let to = dst.join(entry.file_name());

        if file_type.is_dir() {
            copy_dir_all(&from, &to)?;
        } else if file_type.is_file() {
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&from, &to)?;
        }
    }

    Ok(())
}

fn unzip_to_dir(zip_path: &Path, out_dir: &Path) -> AnyResult<()> {
    let file = fs::File::open(zip_path)
        .with_context(|| format!("failed to open zip file {}", zip_path.display()))?;
    let mut archive = ZipArchive::new(file).context("failed to read zip archive")?;

    fs::create_dir_all(out_dir)?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let enclosed = entry
            .enclosed_name()
            .ok_or_else(|| anyhow!("zip contains invalid path"))?
            .to_path_buf();

        let out_path = out_dir.join(enclosed);

        if entry.name().ends_with('/') {
            fs::create_dir_all(&out_path)?;
            continue;
        }

        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut outfile = fs::File::create(&out_path)?;
        io::copy(&mut entry, &mut outfile)?;
    }

    Ok(())
}

fn detect_package_root(unpack_dir: &Path) -> AnyResult<PathBuf> {
    if unpack_dir.join("manifest.json").exists() {
        return Ok(unpack_dir.to_path_buf());
    }

    let mut candidates = Vec::new();

    for entry in fs::read_dir(unpack_dir)? {
        let entry = entry?;
        let path = entry.path();

        if should_ignore_root_entry(&path) {
            continue;
        }

        candidates.push(path);
    }

    if candidates.len() == 1 && candidates[0].is_dir() {
        let candidate = &candidates[0];
        if candidate.join("manifest.json").exists() {
            return Ok(candidate.clone());
        }
    }

    Err(anyhow!(
        "zip must contain manifest.json at root or inside a single top-level folder"
    ))
}

fn read_manifest(package_root: &Path) -> AnyResult<SageAppPackageManifest> {
    let manifest_path = package_root.join("manifest.json");
    let text = fs::read_to_string(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;

    let manifest: SageAppPackageManifest =
        serde_json::from_str(&text).context("failed to parse manifest.json")?;

    if manifest.name.trim().is_empty() {
        return Err(anyhow!("manifest name cannot be empty"));
    }

    if manifest.version.trim().is_empty() {
        return Err(anyhow!("manifest version cannot be empty"));
    }

    Ok(manifest)
}

fn validate_package_structure(package_root: &Path) -> AnyResult<()> {
    let icon = package_root.join("icon.png");
    let entry = package_root.join("dist").join("index.html");

    if !icon.is_file() {
        return Err(anyhow!("package is missing icon.png"));
    }

    if !entry.is_file() {
        return Err(anyhow!("package is missing dist/index.html"));
    }

    Ok(())
}

fn installed_metadata_path(install_dir: &Path) -> PathBuf {
    install_dir.join(INSTALLED_METADATA_FILE)
}

fn read_installed_app_from_dir(dir: &Path) -> AnyResult<InstalledSageApp> {
    let path = installed_metadata_path(dir);
    let text = fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let app: InstalledSageApp =
        serde_json::from_str(&text).context("failed to parse installed app metadata")?;
    Ok(app)
}

fn write_installed_app_metadata(app: &InstalledSageApp, install_dir: &Path) -> AnyResult<()> {
    let path = installed_metadata_path(install_dir);
    let text = serde_json::to_string_pretty(app)
        .map_err(|err| anyhow!("failed to serialize installed app metadata: {err}"))?;
    fs::write(path, format!("{text}\n"))?;
    Ok(())
}

fn list_installed_apps_internal(root: &Path) -> AnyResult<Vec<InstalledSageApp>> {
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut apps = Vec::new();

    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();

        if !entry.file_type()?.is_dir() {
            continue;
        }

        if path
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.starts_with(".tmp-"))
            .unwrap_or(false)
        {
            continue;
        }

        let metadata_path = installed_metadata_path(&path);
        if !metadata_path.is_file() {
            continue;
        }

        apps.push(read_installed_app_from_dir(&path)?);
    }

    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(apps)
}

#[command]
#[specta::specta]
pub async fn list_installed_apps(state: State<'_, AppState>) -> Result<Vec<InstalledSageApp>> {
    let base_path = {
        let state = state.lock().await;
        state.path.clone()
    };

    let root = apps_root(&base_path);

    fs::create_dir_all(&root).map_err(|err| {
        io::Error::other(format!(
            "failed to create apps directory {}: {err}",
            root.display()
        ))
    })?;

    list_installed_apps_internal(&root).map_err(|err| {
        io::Error::other(format!("failed to list installed apps: {err}")).into()
    })
}

#[command]
#[specta::specta]
pub async fn install_app_zip(
    state: State<'_, AppState>,
    zip_path: String,
) -> Result<InstalledSageApp> {
    let base_path = {
        let state = state.lock().await;
        state.path.clone()
    };

    let root = apps_root(&base_path);

    fs::create_dir_all(&root).map_err(|err| {
        io::Error::other(format!(
            "failed to create apps directory {}: {err}",
            root.display()
        ))
    })?;

    let unpack_dir = root.join(format!(".tmp-{}", current_millis()));
    if unpack_dir.exists() {
        fs::remove_dir_all(&unpack_dir).map_err(|err| {
            io::Error::other(format!(
                "failed to remove temporary unpack directory {}: {err}",
                unpack_dir.display()
            ))
        })?;
    }

    let result = (|| -> AnyResult<InstalledSageApp> {
        unzip_to_dir(Path::new(&zip_path), &unpack_dir)?;
        let package_root = detect_package_root(&unpack_dir)?;
        let manifest = read_manifest(&package_root)?;
        validate_package_structure(&package_root)?;

        let existing = list_installed_apps_internal(&root)?
            .into_iter()
            .find(|app| app.name == manifest.name);

        let app_id = existing
            .map(|app| app.id)
            .unwrap_or_else(|| generate_app_id(&manifest.name));

        let install_dir = root.join(&app_id);
        if install_dir.exists() {
            fs::remove_dir_all(&install_dir).with_context(|| {
                format!("failed to remove existing install dir {}", install_dir.display())
            })?;
        }

        copy_dir_all(&package_root, &install_dir)?;

        let installed = InstalledSageApp {
            id: app_id,
            name: manifest.name,
            version: manifest.version,
            install_dir: install_dir.to_string_lossy().to_string(),
            entry_file: install_dir
                .join("dist")
                .join("index.html")
                .to_string_lossy()
                .to_string(),
            icon_file: install_dir.join("icon.png").to_string_lossy().to_string(),
            permissions: manifest.permissions,
        };

        write_installed_app_metadata(&installed, &install_dir)?;
        Ok(installed)
    })();

    if unpack_dir.exists() {
        let _ = fs::remove_dir_all(&unpack_dir);
    }

    result.map_err(|err| {
        io::Error::other(format!("failed to install app zip {}: {err}", zip_path)).into()
    })
}

#[command]
#[specta::specta]
pub async fn uninstall_app(state: State<'_, AppState>, app_id: String) -> Result<()> {
    let base_path = {
        let state = state.lock().await;
        state.path.clone()
    };

    let install_dir = apps_root(&base_path).join(&app_id);
    if install_dir.exists() {
        fs::remove_dir_all(&install_dir).map_err(|err| {
            io::Error::other(format!(
                "failed to remove installed app {} at {}: {err}",
                app_id,
                install_dir.display()
            ))
        })?;
    }

    Ok(())
}

fn app_install_dir(base_path: &Path, app_id: &str) -> PathBuf {
    apps_root(base_path).join(app_id)
}

fn resolve_protocol_file(base_path: &Path, app_id: &str, request_path: &str) -> AnyResult<PathBuf> {
    let install_dir = app_install_dir(base_path, app_id);

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

    let canonical_install_dir = install_dir
        .canonicalize()
        .with_context(|| format!("failed to canonicalize install dir {}", install_dir.display()))?;

    let canonical_path = path
        .canonicalize()
        .with_context(|| format!("failed to canonicalize requested path {}", path.display()))?;

    if !canonical_path.starts_with(&canonical_install_dir) {
        return Err(anyhow!("requested path escapes app directory"));
    }

    if !canonical_path.is_file() {
        return Err(anyhow!("requested file does not exist"));
    }

    Ok(canonical_path)
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

    if request_path.is_empty() || request_path == "/" || request_path == "/index.html" {
        let html = fs::read_to_string(&file_path)
            .with_context(|| format!("failed to read {}", file_path.display()))?;

        let injected = inject_bootstrap_into_index_html(&html, &app);

        return Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/html; charset=utf-8")
            .body(injected.into_bytes())
            .map_err(|err| anyhow!("failed to build protocol response: {err}"));
    }

    let bytes = fs::read(&file_path)
        .with_context(|| format!("failed to read {}", file_path.display()))?;

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

fn html_escape_json_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

fn build_sage_bootstrap(app: &InstalledSageApp) -> String {
    let app_id = html_escape_json_string(&app.id);
    let app_name = html_escape_json_string(&app.name);
    let app_version = html_escape_json_string(&app.version);
    let permissions =
        serde_json::to_string(&app.permissions).unwrap_or_else(|_| "{}".to_string());

    format!(
        r#"<script>
(function () {{
  const __sageAppInfo = {{
    id: {app_id},
    name: {app_name},
    version: {app_version},
    permissions: {permissions},
  }};

  const tauri = window.__TAURI__;
  if (!tauri || !tauri.event || !tauri.webview) {{
    console.warn("Sage bootstrap: Tauri global API is unavailable");
    return;
  }}

  const currentWebview = tauri.webview.getCurrentWebview();
  const sourceLabel = currentWebview.label;

  async function callHost(method, params) {{
    const id = `sage-${{Date.now()}}-${{Math.random().toString(36).slice(2)}}`;

    return new Promise(async (resolve, reject) => {{
      let settled = false;

      const timeoutId = window.setTimeout(() => {{
        if (settled) {{
          return;
        }}
        settled = true;
        unlistenPromise.then((unlisten) => unlisten()).catch(() => {{}});
        reject(new Error(`Sage bridge timeout for ${{method}}`));
      }}, 15000);

      const unlistenPromise = currentWebview.listen('sage-bridge:response', (event) => {{
        const data = event.payload;
        if (!data || data.channel !== 'sage-bridge' || data.id !== id) {{
          return;
        }}

        if (settled) {{
          return;
        }}

        settled = true;
        window.clearTimeout(timeoutId);

        unlistenPromise.then((unlisten) => unlisten()).catch(() => {{}});

        if (data.ok) {{
          resolve(data.result);
        }} else {{
          reject(new Error(data.error?.message || 'Unknown Sage bridge error'));
        }}
      }});

      try {{
        await currentWebview.emitTo('main', 'sage-bridge:request', {{
          sourceLabel,
          appId: __sageAppInfo.id,
          request: {{
            channel: 'sage-bridge',
            id,
            method,
            params
          }}
        }});
      }} catch (error) {{
        if (settled) {{
          return;
        }}

        settled = true;
        window.clearTimeout(timeoutId);
        unlistenPromise.then((unlisten) => unlisten()).catch(() => {{}});
        reject(error instanceof Error ? error : new Error(String(error)));
      }}
    }});
  }}

  window.__SAGE__ = {{
    async bridgePing() {{
      return callHost('bridge.ping');
    }},
    async getAppInfo() {{
      return callHost('app.getInfo');
    }},
    async getPermissions() {{
      return callHost('sage.getPermissions');
    }},
    appInfo: __sageAppInfo,
  }};
}})();
</script>"#
    )
}

fn inject_bootstrap_into_index_html(html: &str, app: &InstalledSageApp) -> String {
    let bootstrap = build_sage_bootstrap(app);

    if let Some(idx) = html.find("<head>") {
        let insert_at = idx + "<head>".len();
        let mut out = String::with_capacity(html.len() + bootstrap.len());
        out.push_str(&html[..insert_at]);
        out.push_str(&bootstrap);
        out.push_str(&html[insert_at..]);
        out
    } else {
        format!("{bootstrap}{html}")
    }
}

fn read_installed_app_by_id(base_path: &Path, app_id: &str) -> AnyResult<InstalledSageApp> {
    let install_dir = app_install_dir(base_path, app_id);
    read_installed_app_from_dir(&install_dir)
}
