pub fn bootstrap_js() -> &'static str {
    r#"
  window.__SAGE__.openWebSocket = async function (input) {
    const params = {
      url: input?.url,
      protocols: Array.isArray(input?.protocols) ? input.protocols : [],
    };
    return callHost('network.wsOpen', params);
  };

  window.__SAGE__.sendWebSocket = async function (input) {
    const params = {
      socketId: input?.socketId,
      text: input?.text,
      base64: input?.base64,
    };
    return callHost('network.wsSend', params);
  };

  window.__SAGE__.closeWebSocket = async function (input) {
    const params = {
      socketId: input?.socketId,
      code: input?.code,
      reason: input?.reason,
    };
    return callHost('network.wsClose', params);
  };
"#
}
