import { invoke } from '@tauri-apps/api/core';
import type { InstalledSageApp } from '@/bindings.ts';

const builtinAppCache = new Map<string, InstalledSageApp | null>();

export async function getBuiltinApp(
  appId: string,
): Promise<InstalledSageApp | undefined> {
  if (builtinAppCache.has(appId)) {
    return builtinAppCache.get(appId) ?? undefined;
  }

  let app: InstalledSageApp | null;

  try {
    app = await invoke<InstalledSageApp | null>('get_builtin_test_app', {
      appId,
    });
  } catch (err) {
    const message =
      err instanceof Error
        ? err.message
        : (() => {
            try {
              return JSON.stringify(err, null, 2);
            } catch {
              return String(err);
            }
          })();

    throw new Error(`Failed to load builtin test app ${appId}: ${message}`);
  }

  builtinAppCache.set(appId, app);
  return app ?? undefined;
}

export function clearBuiltinAppCache(appId?: string) {
  if (appId) {
    builtinAppCache.delete(appId);
    return;
  }

  builtinAppCache.clear();
}
