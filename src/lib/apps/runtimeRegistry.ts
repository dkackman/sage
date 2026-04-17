import { getCurrentWebview, Webview } from '@tauri-apps/api/webview';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { InstalledSageApp } from '@/bindings.ts';

export type SageAppRuntimeState =
  | 'starting'
  | 'running'
  | 'hidden'
  | 'stopping'
  | 'stopped'
  | 'crashed';

export type SageAppRuntimeMode = 'inline';

export interface SageAppRuntimeRecord {
  runtimeId: string;
  appId: string;
  appName: string;
  entrySrc: string;
  webviewLabel: string;
  hostWindowLabel: string;
  mode: SageAppRuntimeMode;
  state: SageAppRuntimeState;
  startedAt: number;
  lastActiveAt: number;
  visible: boolean;
  activeBatchCount: number;
  activeSocketCount: number;
  inFlightRequestCount: number;
}

type RuntimeListener = (records: SageAppRuntimeRecord[]) => void;

type RuntimeInternalRecord = SageAppRuntimeRecord & {
  webview: Webview;
};

const runtimes = new Map<string, RuntimeInternalRecord>();
const runtimeByAppId = new Map<string, string>();
const listeners = new Set<RuntimeListener>();

function emitChange() {
  const snapshot = Array.from(runtimes.values()).map(stripInternal);
  for (const listener of listeners) {
    listener(snapshot);
  }
}

async function emitRuntimeBeforeStop(appId: string) {
  const runtimeId = runtimeByAppId.get(appId);
  if (!runtimeId) {
    return;
  }

  const record = runtimes.get(runtimeId);
  if (!record) {
    return;
  }

  await record.webview.emit('sage-lifecycle:before-stop', {
    appId: record.appId,
    runtimeId: record.runtimeId,
    reason: 'restart',
  });
}

function stripInternal(record: RuntimeInternalRecord): SageAppRuntimeRecord {
  return {
    runtimeId: record.runtimeId,
    appId: record.appId,
    appName: record.appName,
    entrySrc: record.entrySrc,
    webviewLabel: record.webviewLabel,
    hostWindowLabel: record.hostWindowLabel,
    mode: record.mode,
    state: record.state,
    startedAt: record.startedAt,
    lastActiveAt: record.lastActiveAt,
    visible: record.visible,
    activeBatchCount: record.activeBatchCount,
    activeSocketCount: record.activeSocketCount,
    inFlightRequestCount: record.inFlightRequestCount,
  };
}

function runtimeIdFor(appId: string) {
  return `runtime-${appId}`;
}

function inlineLabelFor(appId: string) {
  return `app-inline-${appId}`;
}

function dataStoreIdFor(appId: string): number[] {
  const bytes = new TextEncoder().encode(appId);
  const out = new Uint8Array(16);

  for (let i = 0; i < bytes.length; i += 1) {
    out[i % 16] = (out[i % 16] + bytes[i] + i) & 0xff;
  }

  return Array.from(out);
}

function shouldUseIncognito(app: InstalledSageApp): boolean {
  return !app.grantedPermissions.includes('persistent_storage');
}

async function waitForWebviewCreated(webview: Webview): Promise<void> {
  await new Promise<void>((resolve, reject) => {
    let unlistenCreated: (() => void) | null = null;
    let unlistenError: (() => void) | null = null;

    const cleanupListeners = () => {
      try {
        unlistenCreated?.();
      } catch (err) {
        console.warn('Failed to unlisten created:', err);
      }

      try {
        unlistenError?.();
      } catch (err) {
        console.warn('Failed to unlisten error:', err);
      }
    };

    void (async () => {
      try {
        unlistenCreated = await webview.once('tauri://created', () => {
          cleanupListeners();
          resolve();
        });

        unlistenError = await webview.once('tauri://error', (event) => {
          cleanupListeners();

          const payload =
            typeof event.payload === 'string'
              ? event.payload
              : JSON.stringify(event.payload);
          reject(new Error(payload));
        });
      } catch (err) {
        cleanupListeners();
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
      throw new Error(`Timed out closing webview ${label}`);
    }

    await new Promise((resolve) => window.setTimeout(resolve, 100));
  }
}

export function subscribeAppRuntimes(listener: RuntimeListener): () => void {
  listeners.add(listener);
  listener(Array.from(runtimes.values()).map(stripInternal));
  return () => {
    listeners.delete(listener);
  };
}

export function listAppRuntimes(): SageAppRuntimeRecord[] {
  return Array.from(runtimes.values()).map(stripInternal);
}

export async function closeAppRuntime(
  appId: string,
  options?: { timeoutMs?: number },
): Promise<void> {
  const runtimeId = runtimeByAppId.get(appId);
  if (!runtimeId) {
    return;
  }

  const record = runtimes.get(runtimeId);
  if (!record) {
    runtimeByAppId.delete(appId);
    return;
  }

  const timeoutMs = options?.timeoutMs ?? 8000;
  record.state = 'stopping';
  emitChange();

  try {
    await emitRuntimeBeforeStop(appId);
  } catch (err) {
    console.warn('Failed to emit before-stop lifecycle event:', err);
  }

  try {
    await record.webview.close();
  } catch (err) {
    console.warn('Failed to request webview close:', err);
  }

  await waitForWebviewClosed(record.webviewLabel, timeoutMs);

  runtimes.delete(runtimeId);
  runtimeByAppId.delete(appId);
  emitChange();
}

export async function ensureInlineRuntime(
  app: InstalledSageApp,
): Promise<SageAppRuntimeRecord> {
  const existingRuntimeId = runtimeByAppId.get(app.id);
  if (existingRuntimeId) {
    const existing = runtimes.get(existingRuntimeId);

    if (existing) {
      const liveWebview = await Webview.getByLabel(existing.webviewLabel).catch(
        () => null,
      );

      if (liveWebview) {
        existing.webview = liveWebview;
        existing.lastActiveAt = Date.now();
        existing.visible = true;
        if (existing.state === 'hidden') {
          existing.state = 'running';
        }
        emitChange();
        return stripInternal(existing);
      }

      runtimes.delete(existingRuntimeId);
      runtimeByAppId.delete(app.id);
      emitChange();
    } else {
      runtimeByAppId.delete(app.id);
    }
  }

  const hostWindow = getCurrentWindow();
  const hostWebview = getCurrentWebview();

  const webviewLabel = inlineLabelFor(app.id);
  const entrySrc = `sage-app://${app.id}/index.html`;

  const staleWebview = await Webview.getByLabel(webviewLabel).catch(() => null);
  if (staleWebview) {
    try {
      await staleWebview.close();
    } catch {
      // ignore
    }
    await waitForWebviewClosed(webviewLabel, 8000);
  }
  const isIncognito = shouldUseIncognito(app);

  const webviewOptions = {
    url: entrySrc,
    x: 0,
    y: 0,
    width: 1,
    height: 1,
    focus: true,
    incognito: isIncognito,
    dataStoreIdentifier: dataStoreIdFor(app.id),
  };
  const webview = new Webview(hostWindow, webviewLabel, webviewOptions);

  await waitForWebviewCreated(webview);

  const runtimeId = runtimeIdFor(app.id);
  const record: RuntimeInternalRecord = {
    runtimeId,
    appId: app.id,
    appName: app.name,
    entrySrc,
    webviewLabel,
    hostWindowLabel: hostWebview.label,
    mode: 'inline',
    state: 'running',
    startedAt: Date.now(),
    lastActiveAt: Date.now(),
    visible: true,
    activeBatchCount: 0,
    activeSocketCount: 0,
    inFlightRequestCount: 0,
    webview,
  };

  runtimes.set(runtimeId, record);
  runtimeByAppId.set(app.id, runtimeId);
  emitChange();

  return stripInternal(record);
}

export async function clearAppRuntimeBrowsingData(
  app: InstalledSageApp,
): Promise<void> {
  const webviewLabel = inlineLabelFor(app.id);
  const hostWindow = getCurrentWindow();

  const existingRuntimeId = runtimeByAppId.get(app.id);
  if (existingRuntimeId) {
    await closeAppRuntime(app.id, { timeoutMs: 8000 });
  } else {
    const staleWebview = await Webview.getByLabel(webviewLabel).catch(
      (err: unknown) => {
        throw new Error(
          `Failed to query existing webview ${webviewLabel}: ${String(err)}`,
        );
      },
    );

    if (staleWebview) {
      await staleWebview.close();
      await waitForWebviewClosed(webviewLabel, 8000);
    }
  }

  const clearingWebview = new Webview(hostWindow, webviewLabel, {
    url: `sage-app://${app.id}/__sage/blank`,
    x: 0,
    y: 0,
    width: 1,
    height: 1,
    focus: false,
    incognito: false,
    dataStoreIdentifier: dataStoreIdFor(app.id),
  });

  let created = false;
  let cleared = false;
  let closed = false;

  try {
    await waitForWebviewCreated(clearingWebview);
    created = true;

    await new Promise((resolve) => window.setTimeout(resolve, 150));

    await clearingWebview.clearAllBrowsingData();
    cleared = true;

    await clearingWebview.close();
    await waitForWebviewClosed(webviewLabel, 8000);
    closed = true;
  } catch (err) {
    const parts = [
      `Failed to clear browsing data for app ${app.id}.`,
      `created=${created}`,
      `cleared=${cleared}`,
      `closed=${closed}`,
      `error=${err instanceof Error ? err.message : String(err)}`,
    ];

    try {
      const live = await Webview.getByLabel(webviewLabel);
      if (live) {
        await live.close();
        await waitForWebviewClosed(webviewLabel, 8000);
      }
    } catch (closeErr) {
      parts.push(
        `cleanup_error=${
          closeErr instanceof Error ? closeErr.message : String(closeErr)
        }`,
      );
    }

    throw new Error(parts.join(' '));
  }
}

export async function getRuntimeWebview(
  appId: string,
): Promise<Webview | null> {
  const runtimeId = runtimeByAppId.get(appId);
  if (!runtimeId) {
    return null;
  }

  const record = runtimes.get(runtimeId);
  return record?.webview ?? null;
}

export async function markRuntimeVisible(appId: string, visible: boolean) {
  const runtimeId = runtimeByAppId.get(appId);
  if (!runtimeId) {
    return;
  }

  const record = runtimes.get(runtimeId);
  if (!record) {
    return;
  }

  record.visible = visible;
  record.lastActiveAt = Date.now();

  if (record.state !== 'stopping' && record.state !== 'stopped') {
    record.state = visible ? 'running' : 'hidden';
  }

  try {
    if (visible) {
      await record.webview.show();
    } else {
      await record.webview.hide();
    }
  } catch (err) {
    console.error('Failed to update runtime visibility:', err);
  }

  emitChange();
}

export async function focusRuntime(appId: string) {
  const runtimeId = runtimeByAppId.get(appId);
  if (!runtimeId) {
    return;
  }

  const record = runtimes.get(runtimeId);
  if (!record) {
    return;
  }

  record.visible = true;
  record.state = 'running';
  record.lastActiveAt = Date.now();

  try {
    await record.webview.show();
    await record.webview.setFocus();
  } catch (err) {
    console.error('Failed to focus runtime:', err);
  }

  emitChange();
}

export async function hideRuntime(appId: string) {
  await markRuntimeVisible(appId, false);
}

export async function killRuntime(appId: string) {
  const runtimeId = runtimeByAppId.get(appId);
  if (!runtimeId) {
    return;
  }

  const record = runtimes.get(runtimeId);
  if (!record) {
    runtimeByAppId.delete(appId);
    return;
  }

  try {
    await record.webview.emit('sage-lifecycle:before-stop', {
      reason: 'user_kill',
      appId,
      runtimeId,
    });
  } catch {
    // ignore
  }

  try {
    await closeAppRuntime(appId, { timeoutMs: 8000 });
  } catch {
    // ignore
  }
}
