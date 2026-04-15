use std::{
    fs,
    io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use std::collections::{BTreeMap, BTreeSet};
use anyhow::{Context, Result as AnyResult, anyhow};
use reqwest::Method;
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

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq, PartialOrd, Ord)]
pub struct SageNetworkPermissionEntry {
    pub scheme: String,
    pub host: String,
    #[serde(default)]
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SagePersistentStoragePermission {
    #[serde(default)]
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, Default)]
pub struct SageRequestedPermissions {
    #[serde(default)]
    pub network: Vec<SageNetworkPermissionEntry>,

    #[serde(default)]
    pub persistent_storage: Option<SagePersistentStoragePermission>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, Default)]
pub struct SageGrantedPermissions {
    #[serde(default)]
    pub network: Vec<SageGrantedNetworkPermissionEntry>,

    #[serde(rename = "persistentStorage", alias = "persistent_storage", default)]
    pub persistent_storage: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq, PartialOrd, Ord)]
pub struct SageGrantedNetworkPermissionEntry {
    pub scheme: String,
    pub host: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SageAppPackageManifest {
    pub name: String,
    pub version: String,
    pub permissions: SageRequestedPermissions,
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

    #[serde(rename = "requestedPermissions", alias = "requested_permissions")]
    pub requested_permissions: SageRequestedPermissions,

    #[serde(rename = "grantedPermissions", alias = "granted_permissions")]
    pub granted_permissions: SageGrantedPermissions,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CorruptedInstalledSageApp {
    pub id: String,
    pub install_dir: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ListedSageApp {
    Installed(InstalledSageApp),
    Corrupted(CorruptedInstalledSageApp),
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SageBridgeFetchRequest {
    pub url: String,
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default)]
    pub body: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SageBridgeFetchResponse {
    pub ok: bool,
    pub status: u16,
    pub status_text: String,
    pub headers: BTreeMap<String, String>,
    pub body_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SageBridgeFetchBatchRequest {
    pub requests: Vec<SageBridgeFetchRequest>,
    #[serde(default)]
    pub max_concurrency: Option<usize>,
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

    validate_requested_permissions(&manifest.permissions)?;

    Ok(manifest)
}

fn normalize_scheme(scheme: &str) -> String {
    scheme.trim().to_ascii_lowercase()
}

fn normalize_host(host: &str) -> String {
    host.trim().to_ascii_lowercase()
}

fn validate_network_permission_entry(entry: &SageNetworkPermissionEntry) -> AnyResult<()> {
    let scheme = normalize_scheme(&entry.scheme);
    let host = normalize_host(&entry.host);

    match scheme.as_str() {
        "http" | "https" | "ws" | "wss" => {}
        _ => return Err(anyhow!("unsupported network scheme: {}", entry.scheme)),
    }

    if host.is_empty() {
        return Err(anyhow!("network host cannot be empty"));
    }

    if host == "*" {
        return Ok(());
    }

    if host.starts_with("*.") {
        if host.len() <= 2 || host[2..].contains('*') {
            return Err(anyhow!("invalid wildcard host pattern: {}", entry.host));
        }
        return Ok(());
    }

    if host.contains('*') {
        return Err(anyhow!("only leading wildcard hosts are supported: {}", entry.host));
    }

    Ok(())
}

fn validate_requested_permissions(permissions: &SageRequestedPermissions) -> AnyResult<()> {
    let mut seen = BTreeSet::new();

    for entry in &permissions.network {
        validate_network_permission_entry(entry)?;

        let key = (normalize_scheme(&entry.scheme), normalize_host(&entry.host));
        if !seen.insert(key) {
            return Err(anyhow!(
                "duplicate network permission entry: {} {}",
                entry.scheme,
                entry.host
            ));
        }
    }

    Ok(())
}

fn validate_granted_permissions_against_requested(
    requested: &SageRequestedPermissions,
    granted: &SageGrantedPermissions,
) -> AnyResult<()> {
    let requested_set: BTreeSet<(String, String)> = requested
        .network
        .iter()
        .map(|entry| (normalize_scheme(&entry.scheme), normalize_host(&entry.host)))
        .collect();

    let required_set: BTreeSet<(String, String)> = requested
        .network
        .iter()
        .filter(|entry| entry.required)
        .map(|entry| (normalize_scheme(&entry.scheme), normalize_host(&entry.host)))
        .collect();

    let granted_set: BTreeSet<(String, String)> = granted
        .network
        .iter()
        .map(|entry| (normalize_scheme(&entry.scheme), normalize_host(&entry.host)))
        .collect();

    for key in &granted_set {
        if !requested_set.contains(key) {
            return Err(anyhow!(
                "granted network permission not present in manifest request: {} {}",
                key.0,
                key.1
            ));
        }
    }

    for key in &required_set {
        if !granted_set.contains(key) {
            return Err(anyhow!(
                "missing required network permission grant: {} {}",
                key.0,
                key.1
            ));
        }
    }

    let requested_storage = requested.persistent_storage.is_some();
    let required_storage = requested
        .persistent_storage
        .as_ref()
        .map(|p| p.required)
        .unwrap_or(false);

    if granted.persistent_storage && !requested_storage {
        return Err(anyhow!(
            "persistent storage granted but not requested by manifest"
        ));
    }

    if required_storage && !granted.persistent_storage {
        return Err(anyhow!(
            "missing required persistent storage permission"
        ));
    }

    Ok(())
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

fn list_installed_apps_internal(root: &Path) -> AnyResult<Vec<ListedSageApp>> {
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

        let Some(id) = path
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
        else {
            continue;
        };

        let metadata_path = installed_metadata_path(&path);
        if !metadata_path.is_file() {
            continue;
        }

        match read_installed_app_from_dir(&path) {
            Ok(app) => apps.push(ListedSageApp::Installed(app)),
            Err(err) => apps.push(ListedSageApp::Corrupted(CorruptedInstalledSageApp {
                id,
                install_dir: path.to_string_lossy().to_string(),
                error: err.to_string(),
            })),
        }
    }

    apps.sort_by(|a, b| {
        let a_key = match a {
            ListedSageApp::Installed(app) => app.name.to_lowercase(),
            ListedSageApp::Corrupted(app) => app.id.to_lowercase(),
        };

        let b_key = match b {
            ListedSageApp::Installed(app) => app.name.to_lowercase(),
            ListedSageApp::Corrupted(app) => app.id.to_lowercase(),
        };

        a_key.cmp(&b_key)
    });

    Ok(apps)
}

#[command]
#[specta::specta]
pub async fn preview_app_zip(
    zip_path: String,
) -> Result<SageAppPackageManifest> {
    let unpack_dir = std::env::temp_dir().join(format!(".sage-preview-{}", current_millis()));

    let result = (|| -> AnyResult<SageAppPackageManifest> {
        unzip_to_dir(Path::new(&zip_path), &unpack_dir)?;
        let package_root = detect_package_root(&unpack_dir)?;
        let manifest = read_manifest(&package_root)?;
        validate_package_structure(&package_root)?;
        Ok(manifest)
    })();

    let _ = fs::remove_dir_all(&unpack_dir);

    result.map_err(|err| {
        io::Error::other(format!("failed to preview app zip {}: {err}", zip_path)).into()
    })
}

#[command]
#[specta::specta]
pub async fn list_installed_apps(state: State<'_, AppState>) -> Result<Vec<ListedSageApp>> {
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
    granted_permissions: SageGrantedPermissions,
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
        validate_package_structure(&package_root)?;
        let manifest = read_manifest(&package_root)?;
        validate_granted_permissions_against_requested(&manifest.permissions, &granted_permissions)?;

        let existing = list_installed_apps_internal(&root)?
            .into_iter()
            .find_map(|app| match app {
                ListedSageApp::Installed(installed) if installed.name == manifest.name => {
                    Some(installed)
                }
                _ => None,
            });

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
            requested_permissions: manifest.permissions,
            granted_permissions,
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

#[command]
#[specta::specta]
pub async fn bridge_fetch_http(
    state: State<'_, AppState>,
    app_id: String,
    req: SageBridgeFetchRequest,
) -> Result<SageBridgeFetchResponse> {
    let base_path = {
        let state = state.lock().await;
        state.path.clone()
    };

    let app = read_installed_app_by_id(&base_path, &app_id).map_err(|err| {
        io::Error::other(format!("failed to read installed app {app_id}: {err}"))
    })?;

    let method = req
        .method
        .as_deref()
        .unwrap_or("GET")
        .parse::<Method>()
        .map_err(|err| io::Error::other(format!("invalid HTTP method: {err}")))?;

    let parsed_url = reqwest::Url::parse(&req.url)
        .map_err(|err| io::Error::other(format!("invalid URL: {err}")))?;

    let scheme = parsed_url.scheme().to_string();
    let host = parsed_url
        .host_str()
        .ok_or_else(|| io::Error::other("URL is missing host"))?
        .to_string();

    if !matches!(scheme.as_str(), "http" | "https") {
        return Err(io::Error::other(format!(
            "unsupported fetch scheme: {scheme}"
        ))
            .into());
    }

    if !is_network_allowed_for_app(&app, &scheme, &host) {
        return Err(io::Error::other(format!(
            "network access denied for {scheme}://{host}"
        ))
            .into());
    }

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()?;

    let mut request_builder = client.request(method, parsed_url);

    for (key, value) in req.headers {
        request_builder = request_builder.header(&key, &value);
    }

    if let Some(body) = req.body {
        request_builder = request_builder.body(body);
    }

    let response = request_builder.send().await?;
    let status = response.status();
    let status_text = status
        .canonical_reason()
        .unwrap_or("unknown")
        .to_string();

    let mut headers = BTreeMap::new();
    for (key, value) in response.headers() {
        headers.insert(
            key.to_string(),
            value.to_str().unwrap_or_default().to_string(),
        );
    }

    let body_text = response.text().await?;

    Ok(SageBridgeFetchResponse {
        ok: status.is_success(),
        status: status.as_u16(),
        status_text,
        headers,
        body_text,
    })
}

#[command]
#[specta::specta]
pub async fn bridge_fetch_http_batch(
    state: State<'_, AppState>,
    app_id: String,
    req: SageBridgeFetchBatchRequest,
) -> Result<Vec<SageBridgeFetchResponse>> {
    let mut results = Vec::with_capacity(req.requests.len());

    for request in req.requests {
        let response = bridge_fetch_http(
            state.clone(),
            app_id.clone(),
            request,
        )
            .await?;

        results.push(response);
    }

    Ok(results)
}

fn app_install_dir(base_path: &Path, app_id: &str) -> PathBuf {
    apps_root(base_path).join(app_id)
}

fn resolve_protocol_file(
    base_path: &Path,
    app_id: &str,
    request_path: &str,
) -> AnyResult<PathBuf> {
    let install_dir = app_install_dir(base_path, app_id);

    if request_path == "/__sage_bootstrap__.js" {
        return Ok(install_dir.join("__virtual_sage_bootstrap__.js"));
    }

    let path =
        if request_path.is_empty() || request_path == "/" || request_path == "/index.html" {
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
    let permissions = serde_json::to_string(&app.granted_permissions)
        .unwrap_or_else(|_| "{}".to_string());

    format!(
        r#"(function () {{
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
  const bridgeListeners = new Set();

  currentWebview.listen('sage-bridge:event', (event) => {{
    const data = event.payload;
    if (!data || data.channel !== 'sage-bridge') {{
      return;
    }}

    for (const listener of bridgeListeners) {{
      try {{
        listener(data);
      }} catch (error) {{
        console.error('Sage bridge event listener failed:', error);
      }}
    }}
  }}).catch((error) => {{
    console.error('Failed to subscribe to sage-bridge:event:', error);
  }});

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
    async fetch(input) {{
      const params = {{
        url: input?.url,
        method: input?.method ?? 'GET',
        headers: input?.headers ?? {{}},
        body: input?.body ?? null,
      }};
      return callHost('network.fetch', params);
    }},
    async fetchBatch(input) {{
      const params = {{
        requests: input?.requests ?? [],
        max_concurrency: input?.max_concurrency ?? null,
      }};
      return callHost('network.fetchBatch', params);
    }},
    async openWebSocket(input) {{
      const params = {{
        url: input?.url,
        protocols: Array.isArray(input?.protocols) ? input.protocols : [],
      }};
      return callHost('network.wsOpen', params);
    }},
    async sendWebSocket(input) {{
      const params = {{
        socketId: input?.socketId,
        text: input?.text,
        base64: input?.base64,
      }};
      return callHost('network.wsSend', params);
    }},
    async closeWebSocket(input) {{
      const params = {{
        socketId: input?.socketId,
        code: input?.code,
        reason: input?.reason,
      }};
      return callHost('network.wsClose', params);
    }},
    addEventListener(listener) {{
      bridgeListeners.add(listener);
      return () => {{
        bridgeListeners.delete(listener);
      }};
    }},
    removeEventListener(listener) {{
      bridgeListeners.delete(listener);
    }},
    appInfo: __sageAppInfo,
  }};
}})();
"#
    )
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

fn read_installed_app_by_id(base_path: &Path, app_id: &str) -> AnyResult<InstalledSageApp> {
    let install_dir = app_install_dir(base_path, app_id);
    read_installed_app_from_dir(&install_dir)
}

fn csp_source_list(items: &[&str]) -> String {
    items.join(" ")
}

fn build_app_csp(_app: &InstalledSageApp) -> String {
    let default_src = csp_source_list(&["'self'"]);
    let script_src = csp_source_list(&["'self'", "'wasm-unsafe-eval'"]);
    let style_src = csp_source_list(&["'self'", "'unsafe-inline'"]);
    let img_src = csp_source_list(&["'self'", "data:", "blob:"]);
    let font_src = csp_source_list(&["'self'", "data:"]);
    let media_src = csp_source_list(&["'self'", "data:", "blob:"]);
    let object_src = csp_source_list(&["'none'"]);
    let frame_ancestors = csp_source_list(&["'self'"]);
    let base_uri = csp_source_list(&["'none'"]);
    let form_action = csp_source_list(&["'none'"]);

    let connect_src = csp_source_list(&[
        "'self'",
        "ipc:",
        "ipc://localhost",
        "http://ipc.localhost",
        "https://ipc.localhost",
    ]);

    format!(
        "default-src {default_src}; \
         script-src {script_src}; \
         style-src {style_src}; \
         img-src {img_src}; \
         font-src {font_src}; \
         media-src {media_src}; \
         object-src {object_src}; \
         base-uri {base_uri}; \
         form-action {form_action}; \
         frame-ancestors {frame_ancestors}; \
         connect-src {connect_src}"
    )
}

fn host_matches_pattern(host: &str, pattern: &str) -> bool {
    let host = normalize_host(host);
    let pattern = normalize_host(pattern);

    if pattern == "*" {
        return true;
    }

    if let Some(suffix) = pattern.strip_prefix("*.") {
        return host.ends_with(&format!(".{suffix}"));
    }

    host == pattern
}

fn is_network_allowed_for_app(
    app: &InstalledSageApp,
    scheme: &str,
    host: &str,
) -> bool {
    let scheme = normalize_scheme(scheme);
    let host = normalize_host(host);

    app.granted_permissions.network.iter().any(|entry| {
        normalize_scheme(&entry.scheme) == scheme
            && host_matches_pattern(&host, &entry.host)
    })
}
