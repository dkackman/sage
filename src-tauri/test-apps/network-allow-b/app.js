(async () => {
  const params = new URLSearchParams(window.location.search);
  const runId = params.get('runId');

  if (!runId) {
    throw new Error('missing runId');
  }

  async function tryFetch(url) {
    const controller = new AbortController();
    const timeout = window.setTimeout(() => controller.abort(), 5000);

    try {
      await fetch(url, {
        method: 'GET',
        mode: 'no-cors',
        cache: 'no-store',
        signal: controller.signal,
      });

      window.clearTimeout(timeout);
      return true;
    } catch {
      window.clearTimeout(timeout);
      return false;
    }
  }

  async function report(body) {
    const response = await fetch('sage-app://__sandbox/report/network', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    });

    if (!response.ok) {
      throw new Error(`report failed with status ${response.status}`);
    }
  }

  const allowedUrl = 'https://example.org/';
  const blockedUrl = 'https://example.com/';

  let allowedOk = false;
  let blockedOk = false;
  let error = null;

  try {
    allowedOk = await tryFetch(allowedUrl);
    blockedOk = await tryFetch(blockedUrl);
  } catch (err) {
    error = err instanceof Error ? err.message : String(err);
  }

  await report({
    runId,
    mode: 'allow-b',
    allowedUrl,
    blockedUrl,
    allowedOk,
    blockedOk,
    error,
  });
})().catch(async (err) => {
  try {
    const params = new URLSearchParams(window.location.search);
    await fetch('sage-app://__sandbox/report/network', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        runId: params.get('runId'),
        mode: 'allow-b',
        allowedUrl: 'https://example.org/',
        blockedUrl: 'https://example.com/',
        allowedOk: false,
        blockedOk: false,
        error: err instanceof Error ? err.message : String(err),
      }),
    });
  } catch {
    //
  }
});
