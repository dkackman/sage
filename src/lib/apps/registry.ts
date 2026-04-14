import { InstalledSageApp } from './types';

export const BUILTIN_APPS: InstalledSageApp[] = [];

export function getBuiltinApp(appId: string): InstalledSageApp | undefined {
  return BUILTIN_APPS.find((app) => app.id === appId);
}

