import { getCurrentWebview, Webview } from '@tauri-apps/api/webview';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { LogicalPosition, LogicalSize } from '@tauri-apps/api/dpi';
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

export function getRuntimeByAppId(
  appId: string,
): SageAppRuntimeRecord | undefined {
  const runtimeId = runtimeByAppId.get(appId);
  if (!runtimeId) {
    return undefined;
  }

  const record = runtimes.get(runtimeId);
  return record ? stripInternal(record) : undefined;
}

export async function ensureInlineRuntime(
  app: InstalledSageApp,
): Promise<SageAppRuntimeRecord> {
  const existingRuntimeId = runtimeByAppId.get(app.id);
  if (existingRuntimeId) {
    const existing = runtimes.get(existingRuntimeId);
    if (existing) {
      existing.lastActiveAt = Date.now();
      existing.visible = true;
      if (existing.state === 'hidden') {
        existing.state = 'running';
      }
      emitChange();
      return stripInternal(existing);
    }
  }

  const hostWindow = getCurrentWindow();
  const hostWebview = getCurrentWebview();

  const runtimeId = runtimeIdFor(app.id);
  const webviewLabel = inlineLabelFor(app.id);
  const entrySrc =
    app.source.kind === 'url'
      ? app.source.appUrl
      : `sage-app://${app.id}/index.html`;

  let webview = await Webview.getByLabel(webviewLabel);
  if (!webview) {
    const createdWebview = new Webview(hostWindow, webviewLabel, {
      url: entrySrc,
      x: 0,
      y: 0,
      width: 1,
      height: 1,
      focus: true,
    });

    await new Promise<void>((resolve, reject) => {
      const createdPromise = createdWebview.once('tauri://created', () => {
        resolve();
      });

      const errorPromise = createdWebview.once('tauri://error', (event) => {
        const payload =
          typeof event.payload === 'string'
            ? event.payload
            : JSON.stringify(event.payload);
        reject(new Error(payload));
      });

      void Promise.all([createdPromise, errorPromise]);
    });

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
      // AppHost will place it correctly right after this.
      await record.webview.setSize(new LogicalSize(1, 1));
    } else {
      await record.webview.setPosition(new LogicalPosition(-10000, -10000));
      await record.webview.setSize(new LogicalSize(1, 1));
    }
  } catch (err) {
    console.error('Failed to update runtime visibility:', err);
  }

  emitChange();
}

export function patchRuntimeStats(
  appId: string,
  patch: Partial<
    Pick<
      SageAppRuntimeRecord,
      | 'activeBatchCount'
      | 'activeSocketCount'
      | 'inFlightRequestCount'
      | 'lastActiveAt'
    >
  >,
) {
  const runtimeId = runtimeByAppId.get(appId);
  if (!runtimeId) {
    return;
  }

  const record = runtimes.get(runtimeId);
  if (!record) {
    return;
  }

  Object.assign(record, patch);
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
    return;
  }

  record.state = 'stopping';
  emitChange();

  try {
    await record.webview.emit('sage-lifecycle:before-stop', {
      reason: 'user_kill',
      appId,
      runtimeId,
    });

    await new Promise((resolve) => window.setTimeout(resolve, 1200));
  } catch {
    // ignore
  }

  try {
    await record.webview.close();
  } catch {
    // ignore
  }

  record.state = 'stopped';
  runtimes.delete(runtimeId);
  runtimeByAppId.delete(appId);
  emitChange();
}

