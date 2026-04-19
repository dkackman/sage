import { createContext, useContext, useEffect, useRef } from 'react';
import { useAppsInternal } from '@/hooks/useApps';
import { useAppPendingApprovals } from '@/hooks/useAppPendingApprovals';
import { useBridgeHost } from '@/hooks/useBridgeHost';
import { setForceIncognitoForSecretApps } from '@/lib/apps/runtimeRegistry';

const AppsContext = createContext<ReturnType<typeof useAppsInternal> | null>(
  null,
);

export function AppsProvider({ children }: { children: React.ReactNode }) {
  const value = useAppsInternal();
  const { requestApproval } = useAppPendingApprovals();
  const startedInitialSandboxRunRef = useRef(false);

  const { isReady: bridgeHostReady } = useBridgeHost({
    requestApproval,
  });

  useEffect(() => {
    const clearCycleStatus =
      value.sandboxState?.storageClearCycle?.status ?? null;

    setForceIncognitoForSecretApps(clearCycleStatus === 'failed');
  }, [value.sandboxState]);

  useEffect(() => {
    if (!bridgeHostReady) {
      return;
    }

    if (startedInitialSandboxRunRef.current) {
      return;
    }

    startedInitialSandboxRunRef.current = true;

    void value.rerunSandboxTests();
  }, [bridgeHostReady, value, value.rerunSandboxTests]);

  return <AppsContext.Provider value={value}>{children}</AppsContext.Provider>;
}

export function useApps() {
  const value = useContext(AppsContext);
  if (!value) {
    throw new Error('useApps must be used within AppsProvider');
  }
  return value;
}
