import type { SageApp, SystemSageApp, UserSageApp } from '@/bindings';
import {
  closeAppRuntime,
  ensureInlineRuntime,
} from '@/lib/apps/runtimeRegistry';

type AppLike = SageApp | UserSageApp | SystemSageApp;

export async function restartAppRuntime(
  app: AppLike,
  options?: { visible?: boolean },
) {
  const visible = options?.visible ?? false;

  await closeAppRuntime(app.common.id);
  return await ensureInlineRuntime(app, { visible });
}
