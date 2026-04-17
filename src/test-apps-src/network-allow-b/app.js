import './bridge.js';
import { createSageClient } from './sdk.js';

(async () => {
  const sage = await createSageClient();
  const params = new URLSearchParams(window.location.search);
  const runId = params.get('runId');

  if (!runId) {
    throw new Error('missing runId');
  }

  const allowedUrl = 'https://example.org/';
  const blockedUrl = 'https://example.com/';

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

  let allowedOk = false;
  let blockedOk = false;
  let error = null;

  try {
    allowedOk = await tryFetch(allowedUrl);
    blockedOk = await tryFetch(blockedUrl);
  } catch (err) {
    error = err instanceof Error ? err.message : String(err);
  }

  await sage.app.bridgeSend({
    kind: 'sandbox_report',
    report: {
      type: 'network',
      data: {
        runId,
        mode: 'allow-b',
        allowedUrl,
        blockedUrl,
        allowedOk,
        blockedOk,
        error,
      },
    },
  });
})().catch(async (err) => {
  try {
    const sage = await createSageClient();
    const params = new URLSearchParams(window.location.search);

    await sage.app.bridgeSend({
      kind: 'sandbox_report',
      report: {
        type: 'network',
        data: {
          runId: params.get('runId'),
          mode: 'allow-b',
          allowedUrl: 'https://example.org/',
          blockedUrl: 'https://example.com/',
          allowedOk: false,
          blockedOk: false,
          error: err instanceof Error ? err.message : String(err),
        },
      },
    });
  } catch {}
});
