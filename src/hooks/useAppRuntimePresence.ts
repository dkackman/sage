import { useEffect, useState } from 'react';
import { getRuntimeWebview } from '@/lib/apps/runtimeRegistry';

export function useAppRuntimePresence(appId: string, intervalMs = 1000) {
  const [isRunning, setIsRunning] = useState(false);

  useEffect(() => {
    let cancelled = false;
    let timer: number | null = null;

    const check = async () => {
      try {
        const webview = await getRuntimeWebview(appId);
        if (!cancelled) {
          setIsRunning(!!webview);
        }
      } catch {
        if (!cancelled) {
          setIsRunning(false);
        }
      }
    };

    void check();

    timer = window.setInterval(() => {
      void check();
    }, intervalMs);

    return () => {
      cancelled = true;
      if (timer != null) {
        window.clearInterval(timer);
      }
    };
  }, [appId, intervalMs]);

  return isRunning;
}
