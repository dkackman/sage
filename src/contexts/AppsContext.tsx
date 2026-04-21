import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from 'react';
import {
  commands,
  type InstalledSageApp,
  type ListedSageApp,
  type SageAppUrlPreview,
  type SageGrantedPermissions,
  type SandboxStateView,
} from '@/bindings';
import { useAppPendingApprovals } from '@/hooks/useAppPendingApprovals';
import { useBridgeHost } from '@/hooks/useBridgeHost';
import type { PendingApprovalItem } from '@/hooks/useAppPendingApprovals';

interface PerformAppUpdateOptions {
  restartIfRunning?: boolean;
  visibleAfterRestart?: boolean;
}

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

  getApp: (appId: string) => InstalledSageApp | undefined;
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
  ) => Promise<InstalledSageApp>;
  installUrlApp: (
    appUrl: string,
    grantedPermissions: SageGrantedPermissions,
  ) => Promise<InstalledSageApp>;
  uninstallApp: (appId: string) => Promise<void>;
  checkForUpdate: (appId: string) => Promise<SageAppUrlPreview | null>;
  performAppUpdate: (
    appId: string,
    grantedPermissions: SageGrantedPermissions,
    options?: PerformAppUpdateOptions,
  ) => Promise<InstalledSageApp>;
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
  const [updateAvailability, setUpdateAvailabilityState] = useState<
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

  const refresh = refreshInstalledApps;

  const getApp = useCallback(
    (appId: string) => {
      const found = apps.find(
        (item): item is Extract<ListedSageApp, { kind: 'installed' }> =>
          item.kind === 'installed' && item.id === appId,
      );

      return found;
    },
    [apps],
  );

  const setBusy = useCallback((appId: string, busy: boolean) => {
    setBusyAppIds((prev) => ({
      ...prev,
      [appId]: busy,
    }));
  }, []);

  const setUpdateAvailability = useCallback(
    (
      updater:
        | Record<string, SageAppUrlPreview | null>
        | ((
            prev: Record<string, SageAppUrlPreview | null>,
          ) => Record<string, SageAppUrlPreview | null>),
    ) => {
      setUpdateAvailabilityState((prev) =>
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

        setUpdateAvailabilityState((prev) => {
          const next = { ...prev };
          delete next[appId];
          return next;
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

    setUpdateAvailabilityState((prev) => ({
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
            // Ignore restart failures here; callers still get updated metadata.
          }
        }

        setUpdateAvailabilityState((prev) => ({
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
      setUpdateAvailability,

      installApp,
      installUrlApp,
      uninstallApp,
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
      setUpdateAvailability,
      installApp,
      installUrlApp,
      uninstallApp,
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
