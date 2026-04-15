use crate::apps::{bridge, types::InstalledSageApp};

fn html_escape_json_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

pub fn build_sage_bootstrap(app: &InstalledSageApp) -> String {
    let app_id = html_escape_json_string(&app.id);
    let app_name = html_escape_json_string(&app.name);
    let app_version = html_escape_json_string(&app.version);
    let permissions =
        serde_json::to_string(&app.granted_permissions).unwrap_or_else(|_| "{}".to_string());

    let fragments = bridge::bootstrap_fragments().join("\n");

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

  currentWebview.listen('sage-lifecycle:before-stop', async (event) => {{
    try {{
      window.dispatchEvent(
        new CustomEvent('sage:lifecycle:before-stop', {{
          detail: event.payload,
        }})
      );
    }} catch (error) {{
      console.error('Failed to dispatch before-stop lifecycle event', error);
    }}
  }}).catch((error) => {{
    console.error('Failed to subscribe to sage-lifecycle:before-stop:', error);
  }});
  window.__SAGE__ = {{
    appInfo: __sageAppInfo,
    addEventListener(listener) {{
      bridgeListeners.add(listener);
      return () => {{
        bridgeListeners.delete(listener);
      }};
    }},
    removeEventListener(listener) {{
      bridgeListeners.delete(listener);
    }},
  }};

{fragments}
}})();
"#
    )
}
