use std::{collections::BTreeMap, io};

use reqwest::Method;
use tauri::{command, State};

use crate::{
    app_state::AppState,
    apps::{
        install::read_installed_app_by_id,
        permissions::is_network_allowed_for_app,
        types::{SageBridgeFetchRequest, SageBridgeFetchResponse},
    },
    error::Result,
};

pub fn bootstrap_js() -> &'static str {
    r#"
  window.__SAGE__.fetch = async function (input) {
    const params = {
      url: input?.url,
      method: input?.method ?? 'GET',
      headers: input?.headers ?? {},
      body: input?.body ?? null,
    };
    return callHost('network.fetch', params);
  };
"#
}

pub async fn bridge_fetch_http_inner(
    app_state: AppState,
    app_id: String,
    req: SageBridgeFetchRequest,
) -> Result<SageBridgeFetchResponse> {
    let base_path = {
        let state = app_state.lock().await;
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

    let parsed_url =
        reqwest::Url::parse(&req.url).map_err(|err| io::Error::other(format!("invalid URL: {err}")))?;

    let scheme = parsed_url.scheme().to_string();
    let host = parsed_url
        .host_str()
        .ok_or_else(|| io::Error::other("URL is missing host"))?
        .to_string();

    if !matches!(scheme.as_str(), "http" | "https") {
        return Err(io::Error::other(format!("unsupported fetch scheme: {scheme}")).into());
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
    let status_text = status.canonical_reason().unwrap_or("unknown").to_string();

    let mut headers = BTreeMap::new();
    for (key, value) in response.headers() {
        headers.insert(key.to_string(), value.to_str().unwrap_or_default().to_string());
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
pub async fn bridge_fetch_http(
    state: State<'_, AppState>,
    app_id: String,
    req: SageBridgeFetchRequest,
) -> Result<SageBridgeFetchResponse> {
    bridge_fetch_http_inner(state.inner().clone(), app_id, req).await
}
