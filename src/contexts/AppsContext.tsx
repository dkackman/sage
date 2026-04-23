import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from 'react';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import {
  commands,
  type ListedSageApp,
  type SageAppUrlPreview,
  type SageGrantedPermissions,
  type SandboxStateView,
  SystemSageApp,
  type UserSageApp,
} from '@/bindings';
import { useAppPendingApprovals } from '@/hooks/useAppPendingApprovals';
import { useBridgeHost } from '@/hooks/useBridgeHost';
import type { PendingApprovalItem } from '@/hooks/useAppPendingApprovals';

interface PerformAppUpdateOptions {
  restartIfRunning?: boolean;
  visibleAfterRestart?: boolean;
}

type UserInstalledEntry = { kind: 'user' } & UserSageApp;
type SystemInstalledEntry = { kind: 'system' } & SystemSageApp;
type InstalledEntry = UserInstalledEntry | SystemInstalledEntry;

interface AppsContextValue {
  apps: ListedSageApp[];
  loading: boolean;
  error: string | null;
  busyAppIds: Record<string, boolean>;
  updateAvailability: Record<string, SageAppUrlPreview | null>;
  bridgeHostReady: boolean;
  sandboxState: SandboxStateView | null;

  currentApproval: PendingApprovalItem | null;
  queuedApprovalCount: number;
  currentApprovalSecondsLeft: number;
  approveCurrentApproval: () => void;
  rejectCurrentApproval: () => void;

  getApp: (appId: string) => UserSageApp | undefined;
  refresh: () => Promise<void>;
  refreshInstalledApps: () => Promise<void>;
  setBusy: (appId: string, busy: boolean) => void;
  setUpdateAvailability: (
    updater:
      | Record<string, SageAppUrlPreview | null>
      | ((
          prev: Record<string, SageAppUrlPreview | null>,
        ) => Record<string, SageAppUrlPreview | null>),
  ) => void;

  installApp: (
    zipPath: string,
    grantedPermissions: SageGrantedPermissions,
  ) => Promise<UserSageApp>;
  installUrlApp: (
    appUrl: string,
    grantedPermissions: SageGrantedPermissions,
  ) => Promise<UserSageApp>;
  uninstallApp: (appId: string) => Promise<void>;
  getListedApp: (appId: string) => InstalledEntry | undefined;
  checkForUpdate: (appId: string) => Promise<SageAppUrlPreview | null>;
  performAppUpdate: (
    appId: string,
    grantedPermissions: SageGrantedPermissions,
    options?: PerformAppUpdateOptions,
  ) => Promise<UserSageApp>;
  clearAppStorage: (appId: string) => Promise<void>;
  rerunSandboxTests: () => Promise<SandboxStateView>;
}

const AppsContext = createContext<AppsContextValue | null>(null);

function formatError(err: unknown): string {
  if (err instanceof Error) {
    return err.message;
  }

  if (typeof err === 'string') {
    return err;
  }

  try {
    return JSON.stringify(err, null, 2);
  } catch {
    return String(err);
  }
}

export function AppsProvider({ children }: { children: ReactNode }) {
  const [apps, setApps] = useState<ListedSageApp[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [busyAppIds, setBusyAppIds] = useState<Record<string, boolean>>({});
  const [updateAvailability, setUpdateAvailability] = useState<
    Record<string, SageAppUrlPreview | null>
  >({});
  const [sandboxState, setSandboxState] = useState<SandboxStateView | null>(
    null,
  );

  const {
    currentApproval,
    queuedApprovalCount,
    currentApprovalSecondsLeft,
    requestApproval,
    approveCurrentApproval,
    rejectCurrentApproval,
  } = useAppPendingApprovals();

  const { isReady: bridgeHostReady } = useBridgeHost({
    requestApproval,
  });

  const getListedApp = useCallback(
    (appId: string): InstalledEntry | undefined => {
      return apps.find(
        (item): item is InstalledEntry =>
          (item.kind === 'user' || item.kind === 'system') &&
          item.common.id === appId,
      );
    },
    [apps],
  );

  const refreshInstalledApps = useCallback(async () => {
    setLoading(true);
    setError(null);

    try {
      const [listed, sandbox] = await Promise.all([
        commands.listInstalledApps(),
        commands.appsGetSandboxState(),
      ]);

      setApps(listed);
      setSandboxState(sandbox);
    } catch (err) {
      setError(formatError(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refreshInstalledApps();
  }, [refreshInstalledApps]);

  useEffect(() => {
    let isCancelled = false;
    let unsubscribe: UnlistenFn | null = null;

    const setup = async () => {
      try {
        unsubscribe = await listen<SandboxStateView>(
          'apps:sandbox-state-updated',
          (event) => {
            if (isCancelled) {
              return;
            }

            setSandboxState(event.payload);
          },
        );
      } catch (err) {
        if (!isCancelled) {
          console.error('Failed to subscribe to sandbox state updates:', err);
        }
      }
    };

    void setup();

    return () => {
      isCancelled = true;
      if (unsubscribe) {
        void unsubscribe();
      }
    };
  }, []);

  const currentSandboxRunId = sandboxState?.currentRun?.runId ?? null;

  useEffect(() => {
    if (!currentSandboxRunId) {
      return;
    }

    let isCancelled = false;

    const refreshSandboxState = async () => {
      try {
        const next = await commands.appsGetSandboxState();
        if (!isCancelled) {
          setSandboxState(next);
        }
      } catch (err) {
        if (!isCancelled) {
          console.error('Failed to refresh sandbox state:', err);
        }
      }
    };

    void refreshSandboxState();

    const intervalId = window.setInterval(() => {
      void refreshSandboxState();
    }, 1000);

    return () => {
      isCancelled = true;
      window.clearInterval(intervalId);
    };
  }, [currentSandboxRunId]);

  const refresh = refreshInstalledApps;

  function isUserListedApp(
    entry: ListedSageApp,
  ): entry is { kind: 'user' } & UserSageApp {
    return entry.kind === 'user';
  }

  const getApp = useCallback(
    (appId: string): UserSageApp | undefined => {
      return apps.find(
        (item): item is { kind: 'user' } & UserSageApp =>
          isUserListedApp(item) && item.common.id === appId,
      );
    },
    [apps],
  );

  const setBusy = useCallback((appId: string, busy: boolean) => {
    setBusyAppIds((prev) => ({
      ...prev,
      [appId]: busy,
    }));
  }, []);

  const setUpdateAvailabilityState = useCallback(
    (
      updater:
        | Record<string, SageAppUrlPreview | null>
        | ((
            prev: Record<string, SageAppUrlPreview | null>,
          ) => Record<string, SageAppUrlPreview | null>),
    ) => {
      setUpdateAvailability((prev) =>
        typeof updater === 'function' ? updater(prev) : updater,
      );
    },
    [],
  );

  const installApp = useCallback(
    async (zipPath: string, grantedPermissions: SageGrantedPermissions) => {
      const installed = await commands.installAppZip(
        zipPath,
        grantedPermissions,
      );
      await refreshInstalledApps();
      return installed;
    },
    [refreshInstalledApps],
  );

  const installUrlApp = useCallback(
    async (appUrl: string, grantedPermissions: SageGrantedPermissions) => {
      const installed = await commands.installAppUrl(
        appUrl,
        grantedPermissions,
      );
      await refreshInstalledApps();
      return installed;
    },
    [refreshInstalledApps],
  );

  const uninstallApp = useCallback(
    async (appId: string) => {
      setBusy(appId, true);
      try {
        await commands.uninstallApp(appId);

        setUpdateAvailability((prev) => {
          const next = { ...prev };
          return Object.fromEntries(
            Object.entries(next).filter(([key]) => key !== appId),
          );
        });

        await refreshInstalledApps();
      } finally {
        setBusy(appId, false);
      }
    },
    [refreshInstalledApps, setBusy],
  );

  const checkForUpdate = useCallback(async (appId: string) => {
    const preview = await commands.checkAppUpdate(appId);

    setUpdateAvailability((prev) => ({
      ...prev,
      [appId]: preview,
    }));

    return preview;
  }, []);

  const performAppUpdate = useCallback(
    async (
      appId: string,
      grantedPermissions: SageGrantedPermissions,
      options?: PerformAppUpdateOptions,
    ) => {
      setBusy(appId, true);
      try {
        await commands.downloadAppUpdate(appId);

        const installed = await commands.applyAppUpdate(
          appId,
          grantedPermissions,
        );

        if (options?.restartIfRunning) {
          const { restartAppRuntime } =
            await import('@/lib/apps/restartAppRuntime');

          try {
            await restartAppRuntime(installed, {
              visible: options.visibleAfterRestart ?? true,
            });
          } catch {
            //
          }
        }

        setUpdateAvailability((prev) => ({
          ...prev,
          [appId]: null,
        }));

        await refreshInstalledApps();
        return installed;
      } finally {
        setBusy(appId, false);
      }
    },
    [refreshInstalledApps, setBusy],
  );

  const clearAppStorage = useCallback(
    async (appId: string) => {
      await commands.appsClearRuntimeBrowsingData(appId);
      await refreshInstalledApps();
    },
    [refreshInstalledApps],
  );

  const rerunSandboxTests = useCallback(async () => {
    const next = await commands.appsRerunSandboxTests();
    setSandboxState(next);
    return next;
  }, []);

  const value = useMemo<AppsContextValue>(
    () => ({
      apps,
      loading,
      error,
      busyAppIds,
      updateAvailability,
      bridgeHostReady,
      sandboxState,

      currentApproval,
      queuedApprovalCount,
      currentApprovalSecondsLeft,
      approveCurrentApproval,
      rejectCurrentApproval,

      getApp,
      refresh,
      refreshInstalledApps,
      setBusy,
      setUpdateAvailability: setUpdateAvailabilityState,

      installApp,
      installUrlApp,
      uninstallApp,
      getListedApp,
      checkForUpdate,
      performAppUpdate,
      clearAppStorage,
      rerunSandboxTests,
    }),
    [
      apps,
      loading,
      error,
      busyAppIds,
      updateAvailability,
      bridgeHostReady,
      sandboxState,
      currentApproval,
      queuedApprovalCount,
      currentApprovalSecondsLeft,
      approveCurrentApproval,
      rejectCurrentApproval,
      getApp,
      refresh,
      refreshInstalledApps,
      setBusy,
      setUpdateAvailabilityState,
      installApp,
      installUrlApp,
      uninstallApp,
      getListedApp,
      checkForUpdate,
      performAppUpdate,
      clearAppStorage,
      rerunSandboxTests,
    ],
  );

  return <AppsContext.Provider value={value}>{children}</AppsContext.Provider>;
}

export function useApps() {
  const value = useContext(AppsContext);
  if (!value) {
    throw new Error('useApps must be used within AppsProvider');
  }
  return value;
}
