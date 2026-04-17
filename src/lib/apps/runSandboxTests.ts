import { invoke } from '@tauri-apps/api/core';
import { Webview } from '@tauri-apps/api/webview';
import { getCurrentWindow } from '@tauri-apps/api/window';
import {
  buildIsolationOnlySandboxState,
  type SandboxIsolationProbeResult,
  type SandboxState,
} from '@/lib/apps/sandbox';

const PROBE_LOCAL_STORAGE_KEY = 'sage_probe_local_storage';
const PROBE_COOKIE_KEY = 'sage_probe_cookie';
const PROBE_INDEXED_DB_NAME = 'sage_probe_db';
const PROBE_INDEXED_DB_STORE = 'probe_store';
const PROBE_INDEXED_DB_KEY = 'sage_probe_key';

function uniqueRunId(): string {
  return `sandbox-run-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

async function writeSageProbeIndexedDb(runId: string): Promise<void> {
  await new Promise<void>((resolve, reject) => {
    const open = indexedDB.open(PROBE_INDEXED_DB_NAME);

    open.onerror = () => {
      reject(new Error('Failed to open Sage probe IndexedDB.'));
    };

    open.onupgradeneeded = () => {
      const db = open.result;
      if (!db.objectStoreNames.contains(PROBE_INDEXED_DB_STORE)) {
        db.createObjectStore(PROBE_INDEXED_DB_STORE);
      }
    };

    open.onsuccess = () => {
      try {
        const db = open.result;
        const tx = db.transaction(PROBE_INDEXED_DB_STORE, 'readwrite');
        const store = tx.objectStore(PROBE_INDEXED_DB_STORE);

        const put = store.put(runId, PROBE_INDEXED_DB_KEY);

        put.onerror = () => {
          db.close();
          reject(new Error('Failed to write Sage probe IndexedDB record.'));
        };

        put.onsuccess = () => {
          tx.oncomplete = () => {
            db.close();
            resolve();
          };

          tx.onerror = () => {
            db.close();
            reject(
              new Error('Failed to commit Sage probe IndexedDB transaction.'),
            );
          };
        };
      } catch (err) {
        reject(err instanceof Error ? err : new Error(String(err)));
      }
    };
  });
}

async function writeSageProbes(runId: string): Promise<void> {
  localStorage.setItem(PROBE_LOCAL_STORAGE_KEY, runId);
  document.cookie = `${PROBE_COOKIE_KEY}=${encodeURIComponent(runId)}; path=/`;
  await writeSageProbeIndexedDb(runId);
}

async function waitForWebviewCreated(webview: Webview): Promise<void> {
  await new Promise<void>((resolve, reject) => {
    let unlistenCreated: (() => void) | null = null;
    let unlistenError: (() => void) | null = null;

    const cleanup = () => {
      try {
        unlistenCreated?.();
      } catch {
        // ignore
      }

      try {
        unlistenError?.();
      } catch {
        // ignore
      }
    };

    void (async () => {
      try {
        unlistenCreated = await webview.once('tauri://created', () => {
          cleanup();
          resolve();
        });

        unlistenError = await webview.once('tauri://error', (event) => {
          cleanup();
          const payload =
            typeof event.payload === 'string'
              ? event.payload
              : JSON.stringify(event.payload);
          reject(new Error(payload));
        });
      } catch (err) {
        cleanup();
        reject(err instanceof Error ? err : new Error(String(err)));
      }
    })();
  });
}

async function waitForWebviewClosed(
  label: string,
  timeoutMs = 8000,
): Promise<void> {
  const startedAt = Date.now();

  for (;;) {
    const live = await Webview.getByLabel(label).catch(() => null);
    if (!live) {
      return;
    }

    if (Date.now() - startedAt >= timeoutMs) {
      throw new Error(`Timed out closing sandbox probe webview ${label}`);
    }

    await new Promise((resolve) => window.setTimeout(resolve, 100));
  }
}

async function openProbeWebview(args: {
  label: string;
  runId: string;
  mode: 'incognito' | 'persistent';
  persistentStorage: boolean;
}): Promise<Webview> {
  const hostWindow = getCurrentWindow();

  const stale = await Webview.getByLabel(args.label).catch(() => null);
  if (stale) {
    try {
      await stale.close();
    } catch {
      // ignore
    }
    await waitForWebviewClosed(args.label);
  }

  const url = `sage-app://__sandbox/isolation-check?runId=${encodeURIComponent(
    args.runId,
  )}&mode=${encodeURIComponent(args.mode)}&persistentStorage=${
    args.persistentStorage ? '1' : '0'
  }`;

  const webview = new Webview(hostWindow, args.label, {
    url,
    x: 0,
    y: 0,
    width: 1,
    height: 1,
    focus: false,
    incognito: !args.persistentStorage,
  });

  await waitForWebviewCreated(webview);
  return webview;
}

async function pollIsolationResults(
  runId: string,
  timeoutMs = 10000,
): Promise<SandboxIsolationProbeResult[]> {
  const startedAt = Date.now();

  for (;;) {
    const results = await invoke<SandboxIsolationProbeResult[]>(
      'sandbox_get_run_results',
      { runId },
    );

    if (results.length >= 2) {
      return results;
    }

    if (Date.now() - startedAt >= timeoutMs) {
      throw new Error('Timed out waiting for sandbox isolation probe results.');
    }

    await new Promise((resolve) => window.setTimeout(resolve, 100));
  }
}

export async function runSandboxTests(): Promise<SandboxState> {
  const runId = uniqueRunId();

  await writeSageProbes(runId);
  await invoke('sandbox_reset_run', { runId });

  const incognitoLabel = `sandbox-probe-incognito-${runId}`;
  const persistentLabel = `sandbox-probe-persistent-${runId}`;

  let incognitoWebview: Webview | null = null;
  let persistentWebview: Webview | null = null;

  try {
    [incognitoWebview, persistentWebview] = await Promise.all([
      openProbeWebview({
        label: incognitoLabel,
        runId,
        mode: 'incognito',
        persistentStorage: false,
      }),
      openProbeWebview({
        label: persistentLabel,
        runId,
        mode: 'persistent',
        persistentStorage: true,
      }),
    ]);

    const results = await pollIsolationResults(runId);

    const failed = results.find(
      (result) =>
        result.localStorageVisible ||
        result.cookieVisible ||
        result.indexedDbVisible ||
        !!result.error,
    );

    if (failed) {
      return buildIsolationOnlySandboxState(
        false,
        `Mode ${failed.mode} was able to observe Sage probe data or reported an error.`,
      );
    }

    return buildIsolationOnlySandboxState(
      true,
      'Both sandbox probe modes were unable to observe Sage probe data.',
    );
  } catch (err) {
    return buildIsolationOnlySandboxState(
      false,
      err instanceof Error ? err.message : String(err),
    );
  } finally {
    for (const entry of [incognitoWebview, persistentWebview]) {
      if (!entry) {
        continue;
      }

      try {
        await entry.close();
        await waitForWebviewClosed(entry.label, 8000);
      } catch {
        // ignore
      }
    }
  }
}
