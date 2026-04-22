import { invoke } from '@tauri-apps/api/core';
import { getCurrentWebview, Webview } from '@tauri-apps/api/webview';
import type { SageApp, SystemSageApp, UserSageApp } from '@/bindings.ts';

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
  runtimeKind: 'user' | 'system';
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
  internal: boolean;
  activeBatchCount: number;
  activeSocketCount: number;
  inFlightRequestCount: number;
}

type RuntimeListener = (records: SageAppRuntimeRecord[]) => void;

type RuntimeInternalRecord = SageAppRuntimeRecord & {
  webview: Webview;
};

type AppLike = SageApp | UserSageApp | SystemSageApp;

const runtimes = new Map<string, RuntimeInternalRecord>();
export const runtimeByAppId = new Map<string, string>();
const listeners = new Set<RuntimeListener>();

function isSystemAppLike(app: AppLike): boolean {
  if ('kind' in app) {
    return app.kind === 'system';
  }

  return !('source' in app);
}

function isBuiltinTestApp(app: AppLike): boolean {
  return app.common.id.startsWith('__sage_test_');
}

function shouldDebugTestAppWindows(app: AppLike): boolean {
  return (
    import.meta.env.DEV &&
    import.meta.env.VITE_SAGE_DEBUG_TEST_APPS === '1' &&
    isBuiltinTestApp(app)
  );
}

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
    runtimeKind: record.runtimeKind,
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
    internal: record.internal,
    activeBatchCount: record.activeBatchCount,
    activeSocketCount: record.activeSocketCount,
    inFlightRequestCount: record.inFlightRequestCount,
  };
}

export function inlineLabelFor(appId: string, runtimeKind: 'user' | 'system') {
  return runtimeKind === 'system'
    ? `system-app-inline-${appId}`
    : `app-inline-${appId}`;
}

export function shouldUseIncognito(app: AppLike): boolean {
  const hasPersistentStorage =
    app.common.grantedPermissions.capabilities.includes('persistent_storage');

  if (!hasPersistentStorage) {
    return true;
  }

  if (app.common.capabilityFlags.storageMayContainSecrets) {
    return true;
  }

  return (
    app.common.capabilityFlags.hasSecretAccess
  );
}

export async function waitForWebviewClosed(
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

async function createInlineRuntime(
  app: AppLike,
  options?: {
    visible?: boolean;
    internal?: boolean;
    query?: Record<string, string>;
    path?: string;
  },
): Promise<SageAppRuntimeRecord> {
  const hostWebview = getCurrentWebview();
  const runtimeKind = isSystemAppLike(app) ? 'system' : 'user';
  const webviewLabel = inlineLabelFor(app.common.id, runtimeKind);

  const staleWebview = await Webview.getByLabel(webviewLabel).catch(() => null);
  if (staleWebview) {
    try {
      await staleWebview.close();
    } catch {
      //
    }
    await waitForWebviewClosed(webviewLabel, 8000);
  }

  const isIncognito = shouldUseIncognito(app);

  if (!isIncognito && app.common.capabilityFlags.hasSecretAccess) {
    await invoke('apps_mark_storage_may_contain_secrets', {
      appId: app.common.id,
    });
  }

  const record = await invoke<SageAppRuntimeRecord>(
    'apps_create_inline_runtime',
    {
      args: {
        appId: app.common.id,
        visible: options?.visible ?? true,
        internal: options?.internal ?? false,
        debugLayout: shouldDebugTestAppWindows(app),
        query: options?.query ?? {},
        path: options?.path ?? null,
      },
    },
  );

  const webview = await Webview.getByLabel(webviewLabel).catch(() => null);
  if (!webview) {
    throw new Error(
      `Host created runtime ${webviewLabel}, but no webview handle was found`,
    );
  }

  const internalRecord: RuntimeInternalRecord = {
    ...record,
    hostWindowLabel: hostWebview.label,
    webview,
  };

  runtimes.set(record.runtimeId, internalRecord);
  runtimeByAppId.set(app.common.id, record.runtimeId);
  emitChange();

  return stripInternal(internalRecord);
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
  app: AppLike,
): Promise<SageAppRuntimeRecord> {
  const existingRuntimeId = runtimeByAppId.get(app.common.id);
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
      runtimeByAppId.delete(app.common.id);
      emitChange();
    } else {
      runtimeByAppId.delete(app.common.id);
    }
  }

  return createInlineRuntime(app, { visible: true, internal: false });
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
    //
  }

  try {
    await closeAppRuntime(appId, { timeoutMs: 8000 });
  } catch {
    //
  }
}
