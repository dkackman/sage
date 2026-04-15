import { invoke } from '@tauri-apps/api/core';
import { useCallback, useEffect, useMemo, useState } from 'react';
import {
  InstalledSageApp,
  ListedSageApp,
  SageAppPackageManifest,
  SageAppUrlPreview,
  SageGrantedPermissions,
} from '@/bindings.ts';

export function useApps() {
  const [apps, setApps] = useState<ListedSageApp[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const result = await invoke<ListedSageApp[]>('list_installed_apps');
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

  const previewAppZip = useCallback(async (zipPath: string) => {
    return await invoke<SageAppPackageManifest>('preview_app_zip', {
      zipPath,
    });
  }, []);

  const previewAppUrl = useCallback(async (appUrl: string) => {
    return await invoke<SageAppUrlPreview>('preview_app_url', {
      appUrl,
    });
  }, []);

  const installApp = useCallback(
    async (zipPath: string, permissions: SageGrantedPermissions) => {
      await invoke<InstalledSageApp>('install_app_zip', {
        zipPath,
        grantedPermissions: permissions,
      });
      await refresh();
    },
    [refresh],
  );

  const installAppUrl = useCallback(
    async (appUrl: string, permissions: SageGrantedPermissions) => {
      await invoke<InstalledSageApp>('install_app_url', {
        appUrl,
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
      const app = apps.find((app) => app.id === appId);
      return app?.kind === 'installed' ? app : undefined;
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
    return [...apps].sort((a, b) => {
      const aKey =
        a.kind === 'installed' ? a.name.toLowerCase() : a.id.toLowerCase();

      const bKey =
        b.kind === 'installed' ? b.name.toLowerCase() : b.id.toLowerCase();

      return aKey.localeCompare(bKey);
    });
  }, [apps]);

  return {
    apps: sortedApps,
    loading,
    error,
    refresh,
    previewAppZip,
    previewAppUrl,
    installApp,
    installAppUrl,
    uninstallApp,
    isInstalled,
    getApp,
  };
}
