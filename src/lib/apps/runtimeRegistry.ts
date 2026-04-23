import {
  commands,
  type SageApp,
  type SageAppRuntimeRecord,
  type SystemSageApp,
  type UserSageApp,
} from '@/bindings';

type AppLike = SageApp | UserSageApp | SystemSageApp;

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

export function shouldUseIncognito(app: AppLike): boolean {
  const hasPersistentStorage =
    app.common.grantedPermissions.capabilities.includes('persistent_storage');

  if (!hasPersistentStorage) {
    return true;
  }

  if (app.common.capabilityFlags.storageMayContainSecrets) {
    return true;
  }

  return app.common.capabilityFlags.hasSecretAccess;
}

function buildSystemBridgeRequest(method: string, params?: unknown) {
  return {
    channel: 'sage-system-bridge' as const,
    bridgeVersion: 'v1' as const,
    id: `sage-system-${Date.now()}-${Math.random().toString(36).slice(2)}`,
    method,
    paramsJson: params === undefined ? null : JSON.stringify(params),
  };
}

async function invokeSystemBridge<T>(
  method: string,
  params?: unknown,
): Promise<T> {
  const result = await commands.appsInvokeSystemBridge(
    buildSystemBridgeRequest(method, params),
  );

  if (result.kind === 'pending') {
    throw new Error(
      `System bridge method ${method} unexpectedly returned pending`,
    );
  }

  const response = result.response;

  if ('error' in response) {
    throw new Error(response.error.message);
  }

  return JSON.parse(response.resultJson) as T;
}

export async function createInlineRuntime(
  app: AppLike,
  options?: {
    visible?: boolean;
    internal?: boolean;
    query?: Record<string, string>;
    path?: string;
  },
): Promise<SageAppRuntimeRecord> {
  const isIncognito = shouldUseIncognito(app);

  if (!isIncognito && app.common.capabilityFlags.hasSecretAccess) {
    await commands.appsMarkStorageMayContainSecrets(app.common.id);
  }

  return await commands.appsCreateInlineRuntime({
    appId: app.common.id,
    visible: options?.visible ?? true,
    internal: options?.internal ?? false,
    debugLayout: shouldDebugTestAppWindows(app),
    query: options?.query ?? {},
    path: options?.path ?? null,
  });
}

export async function ensureInlineRuntime(
  app: AppLike,
  options?: { visible?: boolean },
): Promise<SageAppRuntimeRecord> {
  return await createInlineRuntime(app, {
    visible: options?.visible ?? true,
    internal: false,
  });
}

export async function listAppRuntimes(): Promise<SageAppRuntimeRecord[]> {
  return await invokeSystemBridge<SageAppRuntimeRecord[]>(
    'system.listRuntimes',
  );
}

export async function focusRuntime(
  appId: string,
): Promise<SageAppRuntimeRecord> {
  return await invokeSystemBridge<SageAppRuntimeRecord>('system.focusRuntime', {
    appId,
  });
}

export async function killRuntime(
  appId: string,
): Promise<{ ok: boolean; appId: string }> {
  return await invokeSystemBridge<{ ok: boolean; appId: string }>(
    'system.killRuntime',
    { appId },
  );
}

export async function closeAppRuntime(appId: string): Promise<void> {
  await killRuntime(appId);
}
