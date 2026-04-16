use crate::apps::types::InstalledSageApp;

fn csp_source_list(items: &[&str]) -> String {
    items.join(" ")
}

pub fn build_app_csp(_app: &InstalledSageApp) -> String {
    // Installed apps are intentionally blocked from direct external browser-level
    // network access. External network access must go through the Sage bridge,
    // where permission enforcement lives. Local app-origin access remains allowed.
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

    let connect_src = csp_source_list(&["'self'", "sage-app:"]);
    let worker_src = csp_source_list(&["'self'"]);

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
