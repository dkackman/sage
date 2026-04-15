import { invoke } from '@tauri-apps/api/core';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { InstalledSageApp, SageAppPermissions } from '@/bindings.ts';

export function useApps() {
  const [apps, setApps] = useState<InstalledSageApp[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const result = await invoke<InstalledSageApp[]>('list_installed_apps');
      setApps(result);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const installApp = useCallback(
    async (zipPath: string, permissions: SageAppPermissions) => {
      await invoke<InstalledSageApp>('install_app_zip', {
        zipPath,
        grantedPermissions: permissions,
      });
      await refresh();
    },
    [refresh],
  );

  const uninstallApp = useCallback(
    async (appId: string) => {
      await invoke('uninstall_app', { appId });
      await refresh();
    },
    [refresh],
  );

  const getApp = useCallback(
    (appId: string): InstalledSageApp | undefined => {
      return apps.find((app) => app.id === appId);
    },
    [apps],
  );

  const isInstalled = useCallback(
    (appId: string): boolean => {
      return apps.some((app) => app.id === appId);
    },
    [apps],
  );

  const sortedApps = useMemo(() => {
    return [...apps].sort((a, b) => a.name.localeCompare(b.name));
  }, [apps]);

  return {
    apps: sortedApps,
    loading,
    error,
    refresh,
    installApp,
    uninstallApp,
    isInstalled,
    getApp,
  };
}

