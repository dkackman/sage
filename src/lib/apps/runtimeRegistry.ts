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

const CLEAR_STUB_PATH = '/__sage_clear__.html';

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

function clearLabelFor(appId: string) {
  return `app-clear-${appId}`;
}

function shouldUseIncognito(app: InstalledSageApp): boolean {
  return !app.grantedPermissions.includes('persistent_storage');
}

function appEntrySrc(appId: string): string {
  return `sage-app://${appId}/index.html`;
}

function clearStubSrc(appId: string): string {
  return `sage-app://${appId}${CLEAR_STUB_PATH}`;
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

  while (true) {
    const existing = await Webview.getByLabel(label).catch(() => null);

    if (!existing) {
      return;
    }

    if (Date.now() - startedAt >= timeoutMs) {
      throw new Error(`Timed out closing webview ${label}`);
    }

    await new Promise((resolve) => window.setTimeout(resolve, 100));
  }
}

async function closeLooseWebviewByLabel(
  label: string,
  timeoutMs = 8000,
): Promise<void> {
  const webview = await Webview.getByLabel(label).catch(() => null);
  if (!webview) {
    return;
  }

  try {
    await webview.close();
  } catch (err) {
    console.warn(`Failed to request close for webview ${label}:`, err);
  }

  await waitForWebviewClosed(label, timeoutMs);
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

  const startedAt = Date.now();

  let closed = false;

  while (!closed) {
    const existing = await Webview.getByLabel(record.webviewLabel).catch(
      () => null,
    );

    if (!existing) {
      runtimes.delete(runtimeId);
      runtimeByAppId.delete(appId);
      emitChange();
      closed = true;
      continue;
    }

    if (Date.now() - startedAt >= timeoutMs) {
      throw new Error(`Timed out closing runtime for ${appId}`);
    }

    await new Promise((resolve) => window.setTimeout(resolve, 100));
  }
}

export async function clearBrowsingDataWithStub(
  app: InstalledSageApp,
): Promise<void> {
  const runtimeId = runtimeByAppId.get(app.id);
  if (runtimeId) {
    await closeAppRuntime(app.id, { timeoutMs: 8000 });
  }

  await closeLooseWebviewByLabel(clearLabelFor(app.id), 8000);

  const hostWindow = getCurrentWindow();
  const clearLabel = clearLabelFor(app.id);

  const clearWebview = new Webview(hostWindow, clearLabel, {
    url: clearStubSrc(app.id),
    x: 0,
    y: 0,
    width: 1,
    height: 1,
    focus: false,
    incognito: false,
  });

  await waitForWebviewCreated(clearWebview);

  try {
    await clearWebview.hide();
  } catch {
    // ignore
  }

  await new Promise((resolve) => window.setTimeout(resolve, 50));

  await clearWebview.clearAllBrowsingData();

  try {
    await clearWebview.close();
  } catch (err) {
    console.warn('Failed to request clear stub webview close:', err);
  }

  await waitForWebviewClosed(clearLabel, 8000);
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

  const runtimeId = runtimeIdFor(app.id);
  const webviewLabel = inlineLabelFor(app.id);
  const entrySrc = appEntrySrc(app.id);

  let webview = await Webview.getByLabel(webviewLabel);
  if (!webview) {
    const createdWebview = new Webview(hostWindow, webviewLabel, {
      url: entrySrc,
      x: 0,
      y: 0,
      width: 1,
      height: 1,
      focus: true,
      incognito: shouldUseIncognito(app),
    });

    await waitForWebviewCreated(createdWebview);
    webview = createdWebview;
  }

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
