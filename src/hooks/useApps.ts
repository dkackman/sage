import { invoke } from '@tauri-apps/api/core';
import { useCallback, useEffect, useMemo, useState } from 'react';
import type {
  InstalledSageApp,
  ListedSageApp,
  SageAppUrlPreview,
} from '@/bindings.ts';
import {
  buildCompletedSandboxState,
  buildInitialSandboxState,
  buildRunningSandboxState,
  evaluateAppLaunchGate,
  type SandboxState,
} from '@/lib/apps/sandbox';
import { runSandboxTests } from '@/lib/apps/sandbox-tests';

type UpdateAvailabilityMap = Record<string, SageAppUrlPreview | null>;

export function useAppsInternal() {
  const [apps, setApps] = useState<ListedSageApp[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [updateAvailability, setUpdateAvailability] =
    useState<UpdateAvailabilityMap>({});
  const [busyAppIds, setBusyAppIds] = useState<Record<string, boolean>>({});
  const [sandboxState, setSandboxState] = useState<SandboxState>(
    buildInitialSandboxState(),
  );

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

  const rerunSandboxTests = useCallback(async () => {
    setSandboxState(buildRunningSandboxState());

    try {
      const next = await runSandboxTests();
      setSandboxState(next);
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

      setSandboxState(
        buildCompletedSandboxState({
          isolation: {
            passed: false,
            details: message,
          },
          persistenceNormal: {
            passed: false,
            details: 'Skipped because sandbox run failed before completion.',
          },
          persistenceIncognito: {
            passed: false,
            details: 'Skipped because sandbox run failed before completion.',
          },
          network: {
            passed: false,
            details: 'Skipped because sandbox run failed before completion.',
          },
        }),
      );
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const installApp = useCallback(
    async (zipPath: string, permissions: string[]) => {
      await invoke<InstalledSageApp>('install_app_zip', {
        zipPath,
        grantedPermissions: permissions,
      });
      await refresh();
    },
    [refresh],
  );

  const installUrlApp = useCallback(
    async (appUrl: string, permissions: string[]) => {
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
    async (appId: string, grantedPermissions: string[]) => {
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

  const performAppUpdate = useCallback(
    async (
      appId: string,
      grantedPermissions: string[],
      options?: { restartIfRunning?: boolean; visibleAfterRestart?: boolean },
    ) => {
      const restartIfRunning = options?.restartIfRunning ?? false;
      const visibleAfterRestart = options?.visibleAfterRestart ?? false;

      const { getRuntimeWebview } = await import('@/lib/apps/runtimeRegistry');
      const { restartAppRuntime } =
        await import('@/lib/apps/restartAppRuntime');

      const wasRunning = restartIfRunning
        ? !!(await getRuntimeWebview(appId))
        : false;

      const preview = await checkForUpdate(appId, false);

      if (preview) {
        await downloadUpdate(appId);
        await refresh();
      }

      const latestApp = getApp(appId);
      if (!latestApp) {
        throw new Error(`App ${appId} no longer exists after refresh`);
      }

      const updatedApp = await applyUpdate(latestApp.id, grantedPermissions);

      if (wasRunning) {
        await restartAppRuntime(updatedApp, {
          visible: visibleAfterRestart,
        });
      }

      await refresh();
      return updatedApp;
    },
    [applyUpdate, checkForUpdate, downloadUpdate, getApp, refresh],
  );

  const clearAppStorage = useCallback(
    async (appId: string) => {
      try {
        setBusy(appId, true);
        await invoke('storage_clear_all_for_app', { appId });
        await refresh();
      } finally {
        setBusy(appId, false);
      }
    },
    [refresh, setBusy],
  );

  const getAppLaunchGate = useCallback(
    (appId: string) => {
      const app = getApp(appId);
      if (!app) {
        return null;
      }

      return evaluateAppLaunchGate(app, sandboxState);
    },
    [getApp, sandboxState],
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
    installUrlApp,
    uninstallApp,
    isInstalled,
    getApp,
    getAppLaunchGate,
    checkForUpdate,
    downloadUpdate,
    applyUpdate,
    performAppUpdate,
    clearAppStorage,
    sandboxState,
    rerunSandboxTests,
    updateAvailability,
    busyAppIds,
  };
}
