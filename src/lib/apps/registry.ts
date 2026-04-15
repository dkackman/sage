import { InstalledSageApp } from '@/bindings.ts';

export const BUILTIN_APPS: InstalledSageApp[] = [];

export function getBuiltinApp(appId: string): InstalledSageApp | undefined {
  return BUILTIN_APPS.find((app) => app.id === appId);
}

