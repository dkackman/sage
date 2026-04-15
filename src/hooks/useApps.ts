import { invoke } from '@tauri-apps/api/core';
import { useCallback, useEffect, useMemo, useState } from 'react';
import type {
  InstalledSageApp,
  ListedSageApp,
  SageAppUrlPreview,
  SageGrantedPermissions,
} from '@/bindings.ts';

type UpdateAvailabilityMap = Record<string, SageAppUrlPreview | null>;

export function useApps() {
  const [apps, setApps] = useState<ListedSageApp[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [updateAvailability, setUpdateAvailability] =
    useState<UpdateAvailabilityMap>({});
  const [busyAppIds, setBusyAppIds] = useState<Record<string, boolean>>({});

  const setBusy = useCallback((appId: string, busy: boolean) => {
    setBusyAppIds((prev) => {
      if (busy) {
        return { ...prev, [appId]: true };
      }

      return Object.fromEntries(
        Object.entries(prev).filter(([key]) => key !== appId),
      );
    });
  }, []);

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

  const uninstallApp = useCallback(
    async (appId: string) => {
      await invoke('uninstall_app', { appId });
      setUpdateAvailability((prev) => {
        return Object.fromEntries(
          Object.entries(prev).filter(([key]) => key !== appId),
        );
      });
      await refresh();
    },
    [refresh],
  );

  const getApp = useCallback(
    (appId: string): InstalledSageApp | undefined => {
      const app = apps.find((entry) => entry.id === appId);
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

  const checkForUpdate = useCallback(
    async (appId: string, forceRefresh = true) => {
      try {
        setBusy(appId, true);
        const preview = await invoke<SageAppUrlPreview | null>(
          'check_app_update',
          { appId },
        );

        setUpdateAvailability((prev) => ({
          ...prev,
          [appId]: preview,
        }));

        if (forceRefresh) {
          await refresh();
        }

        return preview;
      } finally {
        setBusy(appId, false);
      }
    },
    [refresh, setBusy],
  );

  const downloadUpdate = useCallback(
    async (appId: string) => {
      try {
        setBusy(appId, true);
        const installed = await invoke<InstalledSageApp>(
          'download_app_update',
          {
            appId,
          },
        );

        setUpdateAvailability((prev) => ({
          ...prev,
          [appId]: null,
        }));

        await refresh();
        return installed;
      } finally {
        setBusy(appId, false);
      }
    },
    [refresh, setBusy],
  );

  const applyUpdate = useCallback(
    async (appId: string, grantedPermissions: SageGrantedPermissions) => {
      try {
        setBusy(appId, true);
        const installed = await invoke<InstalledSageApp>('apply_app_update', {
          appId,
          grantedPermissions,
        });

        setUpdateAvailability((prev) => ({
          ...prev,
          [appId]: null,
        }));

        await refresh();
        return installed;
      } finally {
        setBusy(appId, false);
      }
    },
    [refresh, setBusy],
  );

  useEffect(() => {
    const urlApps = apps.filter(
      (entry) => entry.kind === 'installed' && entry.source?.kind === 'url',
    );

    if (urlApps.length === 0) {
      return;
    }

    const intervalId = window.setInterval(
      () => {
        urlApps.forEach((entry) => {
          if (busyAppIds[entry.id]) {
            return;
          }

          void invoke<SageAppUrlPreview | null>('check_app_update', {
            appId: entry.id,
          })
            .then((preview) => {
              setUpdateAvailability((prev) => ({
                ...prev,
                [entry.id]: preview,
              }));
            })
            .catch(() => {
              // intentionally quiet
            });
        });
      },
      10 * 60 * 1000,
    );

    return () => {
      window.clearInterval(intervalId);
    };
  }, [apps, busyAppIds]);

  return {
    apps: sortedApps,
    loading,
    error,
    refresh,
    installApp,
    uninstallApp,
    isInstalled,
    getApp,
    checkForUpdate,
    downloadUpdate,
    applyUpdate,
    updateAvailability,
    busyAppIds,
  };
}
