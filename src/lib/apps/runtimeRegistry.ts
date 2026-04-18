import { invoke } from '@tauri-apps/api/core';
import { getCurrentWebview, Webview } from '@tauri-apps/api/webview';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { InstalledSageApp } from '@/bindings.ts';
import { platform } from '@tauri-apps/plugin-os';
import { removeDataStore } from '@tauri-apps/api/app';
import { BaseDirectory, remove } from '@tauri-apps/plugin-fs';

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
  internal: boolean;
  activeBatchCount: number;
  activeSocketCount: number;
  inFlightRequestCount: number;
}

type RuntimeListener = (records: SageAppRuntimeRecord[]) => void;

type RuntimeInternalRecord = SageAppRuntimeRecord & {
  webview: Webview;
};

const runtimes = new Map<string, RuntimeInternalRecord>();
export const runtimeByAppId = new Map<string, string>();
const listeners = new Set<RuntimeListener>();

let forceIncognitoForSecretApps = false;

export function setForceIncognitoForSecretApps(value: boolean) {
  forceIncognitoForSecretApps = value;
}

function isBuiltinTestApp(app: InstalledSageApp): boolean {
  return app.id.startsWith('__sage_test_');
}

function shouldDebugTestAppWindows(app: InstalledSageApp): boolean {
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

export function runtimeIdFor(appId: string) {
  return `runtime-${appId}`;
}

export function inlineLabelFor(appId: string) {
  return `app-inline-${appId}`;
}

/**
 * Windows-only custom data directory for the webview profile.
 * Tauri resolves this relative to appDataDir()/${webviewLabel}.
 */
export function dataDirectoryFor(appId: string) {
  return `profiles/${appId}`;
}

export function dataStoreIdFor(appId: string): number[] {
  const bytes = new TextEncoder().encode(appId);
  const out = new Uint8Array(16);

  for (let i = 0; i < bytes.length; i += 1) {
    out[i % 16] = (out[i % 16] + bytes[i] + i) & 0xff;
  }

  return Array.from(out);
}

export function shouldUseIncognito(app: InstalledSageApp): boolean {
  const hasPersistentStorage = (
    app.grantedPermissions.capabilities
  ).includes('persistent_storage');

  if (!hasPersistentStorage) {
    return true;
  }

  if (app.permissionFlags.storageMayContainSecrets) {
    return true;
  }

  if (forceIncognitoForSecretApps && app.permissionFlags.hasSecretAccess) {
    return true;
  }

  return false;
}

async function supportsDataDirectory(): Promise<boolean> {
  return (await platform()) === 'windows';
}

async function supportsDataStoreIdentifier(): Promise<boolean> {
  const os = await platform();
  return os === 'macos' || os === 'ios';
}

async function buildWebviewOptions(
  app: InstalledSageApp,
  entrySrc: string,
  options?: {
    visible?: boolean;
  },
) {
  const isIncognito = shouldUseIncognito(app);
  const debug = shouldDebugTestAppWindows(app);

  const webviewOptions: ConstructorParameters<typeof Webview>[2] = {
    url: entrySrc,
    x: debug ? 80 : 0,
    y: debug ? 80 : 0,
    width: debug ? 200 : 1,
    height: debug ? 200 : 1,
    focus: debug || !!options?.visible,
    incognito: isIncognito,
  };

  if (!isIncognito && (await supportsDataStoreIdentifier())) {
    webviewOptions.dataStoreIdentifier = dataStoreIdFor(app.id);
  }

  if (!isIncognito && (await supportsDataDirectory())) {
    webviewOptions.dataDirectory = dataDirectoryFor(app.id);
  }

  return webviewOptions;
}

export async function waitForWebviewCreated(webview: Webview): Promise<void> {
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
  app: InstalledSageApp,
  options?: {
    visible?: boolean;
    internal?: boolean;
    query?: Record<string, string>;
    path?: string;
  },
): Promise<SageAppRuntimeRecord> {
  const hostWindow = getCurrentWindow();
  const hostWebview = getCurrentWebview();

  const webviewLabel = inlineLabelFor(app.id);

  const entryPath = options?.path ?? `/${app.entryFile}`;
  const url = new URL(`sage-app://${app.id}${entryPath}`);
  for (const [key, value] of Object.entries(options?.query ?? {})) {
    url.searchParams.set(key, value);
  }
  const entrySrc = url.toString();

  const staleWebview = await Webview.getByLabel(webviewLabel).catch(() => null);
  if (staleWebview) {
    try {
      await staleWebview.close();
    } catch {
      //
    }
    await waitForWebviewClosed(webviewLabel, 8000);
  }

  const debug = shouldDebugTestAppWindows(app);
  const isIncognito = shouldUseIncognito(app);

  if (!isIncognito && app.permissionFlags.hasSecretAccess) {
    await invoke('apps_mark_storage_may_contain_secrets', {
      appId: app.id,
    });
  }

  const webviewOptions = await buildWebviewOptions(app, entrySrc, {
    visible: options?.visible,
  });

  const webview = new Webview(hostWindow, webviewLabel, webviewOptions);

  await waitForWebviewCreated(webview);

  if (!options?.visible && !debug) {
    await webview.hide();
  }

  const runtimeId = runtimeIdFor(app.id);
  const record: RuntimeInternalRecord = {
    runtimeId,
    appId: app.id,
    appName: app.name,
    entrySrc,
    webviewLabel,
    hostWindowLabel: hostWebview.label,
    mode: 'inline',
    state: options?.visible === false ? 'hidden' : 'running',
    startedAt: Date.now(),
    lastActiveAt: Date.now(),
    visible: options?.visible ?? true,
    internal: options?.internal ?? false,
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

  return createInlineRuntime(app, { visible: true, internal: false });
}

export async function startInternalInlineRuntime(
  app: InstalledSageApp,
  options?: { query?: Record<string, string>; path?: string },
): Promise<SageAppRuntimeRecord> {
  const existingRuntimeId = runtimeByAppId.get(app.id);
  if (existingRuntimeId) {
    await closeAppRuntime(app.id, { timeoutMs: 8000 });
  }

  return createInlineRuntime(app, {
    visible: false,
    internal: true,
    query: options?.query,
    path: options?.path,
  });
}

async function removeAppDataStore(app: InstalledSageApp): Promise<void> {
  if (await supportsDataStoreIdentifier()) {
    try {
      await removeDataStore(
        dataStoreIdFor(app.id) as [
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
          number,
        ],
      );
    } catch (err) {
      console.warn(
        `removeDataStore failed for ${app.id}:`,
        err instanceof Error ? err.message : String(err),
      );
    }
  }
}

async function removeAppDataDirectory(app: InstalledSageApp): Promise<void> {
  if (await supportsDataDirectory()) {
    try {
      await remove(dataDirectoryFor(app.id), {
        baseDir: BaseDirectory.AppData,
        recursive: true,
      });
    } catch (err) {
      console.warn(
        `remove data directory failed for ${app.id}:`,
        err instanceof Error ? err.message : String(err),
      );
    }
  }
}

/**
 * Best-effort host-side clear.
 *
 * This does NOT prove the app origin is clean; your verification cycle should
 * still decide whether the capability passed for this environment.
 */
export async function clearAppRuntimeBrowsingData(
  app: InstalledSageApp,
): Promise<void> {
  const webviewLabel = inlineLabelFor(app.id);

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

  await Promise.allSettled([
    removeAppDataStore(app),
    removeAppDataDirectory(app),
  ]);
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
