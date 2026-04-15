pub fn bootstrap_js() -> &'static str {
    r#"
  window.__SAGE__.bridgePing = async function () {
    return callHost('bridge.ping');
  };

  window.__SAGE__.getAppInfo = async function () {
    return callHost('app.getInfo');
  };

  window.__SAGE__.getPermissions = async function () {
    return callHost('sage.getPermissions');
  };
"#
}
