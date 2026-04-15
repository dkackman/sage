import { useEffect, useState } from 'react';
import {
  listAppRuntimes,
  subscribeAppRuntimes,
  type SageAppRuntimeRecord,
} from '@/lib/apps/runtimeRegistry';

export function useAppRuntimes() {
  const [runtimes, setRuntimes] = useState<SageAppRuntimeRecord[]>(() =>
    listAppRuntimes(),
  );

  useEffect(() => {
    return subscribeAppRuntimes(setRuntimes);
  }, []);

  return runtimes;
}

