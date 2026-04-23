import { useEffect, useState } from 'react';
import {
  listAppRuntimes,
  subscribeAppRuntimes,
  type SageAppRuntimeRecord,
} from '@/lib/apps/runtimeRegistry';

export function useAppRuntimes(options?: { includeInternal?: boolean }) {
  const includeInternal = options?.includeInternal ?? false;

  const [runtimes, setRuntimes] = useState<SageAppRuntimeRecord[]>(() => {
    const all = listAppRuntimes();
    return includeInternal ? all : all.filter((runtime) => !runtime.internal);
  });

  useEffect(() => {
    return subscribeAppRuntimes((next: SageAppRuntimeRecord[]) => {
      setRuntimes(
        includeInternal ? next : next.filter((runtime) => !runtime.internal),
      );
    });
  }, [includeInternal]);

  return runtimes;
}
