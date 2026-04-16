use std::collections::BTreeSet;

use crate::apps::types::{
    InstalledSageApp, SageGrantedNetworkPermissionEntry,
};

fn csp_source_list(items: &[String]) -> String {
    items.join(" ")
}

fn network_permission_to_csp_source(
    permission: &SageGrantedNetworkPermissionEntry,
) -> Option<String> {
    let scheme = permission.scheme.trim().to_ascii_lowercase();
    let host = permission.host.trim().to_ascii_lowercase();

    if host.is_empty() {
        return None;
    }

    // Keep this strict. Expand only if you intentionally support more.
    match scheme.as_str() {
        "https" | "wss" => {}
        _ => return None,
    }

    // CSP source expressions should be origins / host-source patterns, not paths.
    // Examples:
    //   https://api.coinset.org
    //   https://*.google.com
    //   wss://relay.walletconnect.com
    //
    // Reject obviously dangerous / malformed host values.
    if host.contains('/') || host.contains('?') || host.contains('#') || host.contains(' ') {
        return None;
    }

    Some(format!("{scheme}://{host}"))
}

pub fn build_app_csp(app: &InstalledSageApp) -> String {
    let default_src = csp_source_list(&vec!["'self'".to_string()]);
    let script_src = csp_source_list(&vec![
        "'self'".to_string(),
        "'wasm-unsafe-eval'".to_string(),
    ]);
    let style_src = csp_source_list(&vec![
        "'self'".to_string(),
        "'unsafe-inline'".to_string(),
    ]);
    let img_src = csp_source_list(&vec![
        "'self'".to_string(),
        "data:".to_string(),
        "blob:".to_string(),
    ]);
    let font_src = csp_source_list(&vec![
        "'self'".to_string(),
        "data:".to_string(),
    ]);
    let media_src = csp_source_list(&vec![
        "'self'".to_string(),
        "data:".to_string(),
        "blob:".to_string(),
    ]);
    let object_src = csp_source_list(&vec!["'none'".to_string()]);
    let frame_ancestors = csp_source_list(&vec!["'self'".to_string()]);
    let base_uri = csp_source_list(&vec!["'none'".to_string()]);
    let form_action = csp_source_list(&vec!["'none'".to_string()]);
    let worker_src = csp_source_list(&vec!["'self'".to_string()]);

    let mut connect_sources = BTreeSet::from([
        "'self'".to_string(),
        "ipc:".to_string(),
        "ipc://localhost".to_string(),
        "http://ipc.localhost".to_string(),
    ]);

    for permission in &app.granted_permissions.network {
        if let Some(source) = network_permission_to_csp_source(permission) {
            connect_sources.insert(source);
        }
    }

    let connect_src = csp_source_list(
        &connect_sources.into_iter().collect::<Vec<_>>(),
    );

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
         connect-src {connect_src}; \
         worker-src {worker_src};"
    )
}
