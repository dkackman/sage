import { useEffect, useState } from 'react';
import {
  listAppRuntimes,
  type SageAppRuntimeRecord,
} from '@/lib/apps/runtimeRegistry';

export function useAppRuntimes(options?: {
  includeInternal?: boolean;
  refreshMs?: number;
}) {
  const includeInternal = options?.includeInternal ?? false;
  const refreshMs = options?.refreshMs ?? 1000;

  const [runtimes, setRuntimes] = useState<SageAppRuntimeRecord[]>([]);

  useEffect(() => {
    let cancelled = false;

    const refresh = async () => {
      try {
        const all = await listAppRuntimes();
        if (cancelled) {
          return;
        }

        setRuntimes(
          includeInternal ? all : all.filter((runtime) => !runtime.internal),
        );
      } catch (err) {
        if (!cancelled) {
          console.error('Failed to load app runtimes:', err);
        }
      }
    };

    void refresh();
    const intervalId = window.setInterval(() => {
      void refresh();
    }, refreshMs);

    return () => {
      cancelled = true;
      window.clearInterval(intervalId);
    };
  }, [includeInternal, refreshMs]);

  return runtimes;
}
