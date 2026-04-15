import type { InstalledSageApp } from '@/bindings';
import {
  closeAppRuntime,
  ensureInlineRuntime,
  markRuntimeVisible,
} from '@/lib/apps/runtimeRegistry';

export async function restartAppRuntime(
  app: InstalledSageApp,
  options?: { visible?: boolean },
) {
  const visible = options?.visible ?? false;

  await closeAppRuntime(app.id, { timeoutMs: 8000 });
  const runtime = await ensureInlineRuntime(app);
  await markRuntimeVisible(app.id, visible);
  return runtime;
}
