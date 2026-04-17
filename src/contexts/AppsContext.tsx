import { createContext, useContext } from 'react';
import { useAppsInternal } from '@/hooks/useApps';

const AppsContext = createContext<ReturnType<typeof useAppsInternal> | null>(
  null,
);

export function AppsProvider({ children }: { children: React.ReactNode }) {
  const value = useAppsInternal();
  return <AppsContext.Provider value={value}>{children}</AppsContext.Provider>;
}

export function useApps() {
  const value = useContext(AppsContext);
  if (!value) {
    throw new Error('useApps must be used within AppsProvider');
  }
  return value;
}
