import { createContext, useContext } from 'react';
import { useSandboxInternal } from '@/hooks/useSandbox';

const SandboxContext = createContext<ReturnType<
  typeof useSandboxInternal
> | null>(null);

export function SandboxProvider({ children }: { children: React.ReactNode }) {
  const value = useSandboxInternal();
  return (
    <SandboxContext.Provider value={value}>{children}</SandboxContext.Provider>
  );
}

export function useSandbox() {
  const value = useContext(SandboxContext);
  if (!value) {
    throw new Error('useSandbox must be used within SandboxProvider');
  }
  return value;
}
