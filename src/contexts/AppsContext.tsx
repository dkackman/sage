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

  useBridgeHost({
    requestApproval,
  });

  useEffect(() => {
    const clearCycleStatus =
      value.sandboxState.capabilities.storage_clear_cycle?.status;

    setForceIncognitoForSecretApps(clearCycleStatus === 'failed');
  }, [value.sandboxState]);

  useEffect(() => {
    if (startedInitialSandboxRunRef.current) {
      return;
    }

    startedInitialSandboxRunRef.current = true;

    const timeoutId = window.setTimeout(() => {
      void value.rerunSandboxTests();
    }, 0);

    return () => {
      window.clearTimeout(timeoutId);
    };
  }, [value, value.rerunSandboxTests]);

  return <AppsContext.Provider value={value}>{children}</AppsContext.Provider>;
}

export function useApps() {
  const value = useContext(AppsContext);
  if (!value) {
    throw new Error('useApps must be used within AppsProvider');
  }
  return value;
}
