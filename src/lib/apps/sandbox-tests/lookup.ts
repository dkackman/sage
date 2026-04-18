import type { SandboxAppResult } from '@/lib/apps/sandboxRuntimeStore';

export function findByAppId<T>(
  items: SandboxAppResult<T>[],
  appId: string,
): T | undefined {
  return items.find((item) => item.appId === appId)?.data;
}
