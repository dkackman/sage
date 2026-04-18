import { TEST_APP_IDS } from '@/lib/apps/testApps';
import {
  closeAppRuntime,
  startInternalInlineRuntime,
} from '@/lib/apps/runtimeRegistry';
import { getBuiltinApp } from '@/lib/apps/registry';

export function uniqueRunId(prefix: string): string {
  return `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

export function formatUnknownError(err: unknown): string {
  if (err instanceof Error) {
    return err.message;
  }

  if (typeof err === 'string') {
    return err;
  }

  try {
    return JSON.stringify(err, null, 2);
  } catch {
    return String(err);
  }
}

export function labelForAppId(appId: string): string {
  switch (appId) {
    case TEST_APP_IDS.storageIsolationPersistent:
      return 'persistent isolation';
    case TEST_APP_IDS.storageIsolationIncognito:
      return 'incognito isolation';
    case TEST_APP_IDS.persistencePersistent:
      return 'persistent';
    case TEST_APP_IDS.persistenceIncognito:
      return 'incognito';
    case TEST_APP_IDS.networkAllowA:
      return 'allow-a';
    case TEST_APP_IDS.networkAllowB:
      return 'allow-b';
    default:
      return appId;
  }
}

export async function startTestApp(
  appId: string,
  query?: Record<string, string>,
): Promise<void> {
  const app = await getBuiltinApp(appId);
  if (!app) {
    throw new Error(`Missing builtin test app ${appId}`);
  }

  await startInternalInlineRuntime(app, { query });
}

export async function stopTestApps(appIds: string[]): Promise<void> {
  await Promise.allSettled(appIds.map((appId) => closeAppRuntime(appId)));
}

export async function pollResults<T>(args: {
  runId: string;
  expectedCount: number;
  timeoutMs: number;
  read: () => T[];
  label: string;
}): Promise<T[]> {
  const startedAt = Date.now();

  for (;;) {
    const results = args.read();

    if (results.length >= args.expectedCount) {
      return results;
    }

    if (Date.now() - startedAt >= args.timeoutMs) {
      throw new Error(`Timed out waiting for ${args.label} results.`);
    }

    await new Promise((resolve) => window.setTimeout(resolve, 100));
  }
}

export function shouldKeepFailedTestAppsOpen(): boolean {
  return (
    import.meta.env.DEV && import.meta.env.VITE_SAGE_DEBUG_TEST_APPS === '1'
  );
}

export async function stopTestAppsOnSuccess(appIds: string[]): Promise<void> {
  await stopTestApps(appIds);
}

export async function stopTestAppsOnFailure(appIds: string[]): Promise<void> {
  if (shouldKeepFailedTestAppsOpen()) {
    return;
  }

  await stopTestApps(appIds);
}
