import React, { useCallback, useEffect } from 'react';
import { platform } from '@tauri-apps/plugin-os';
import { LogicalPosition, LogicalSize } from '@tauri-apps/api/dpi';
import {
  ensureInlineRuntime,
  getRuntimeWebview,
  markRuntimeVisible,
} from '@/lib/apps/runtimeRegistry';
import type { InstalledSageApp } from '@/bindings';
import { getCurrentWindow } from '@tauri-apps/api/window';

async function getMacWindowedTopInsetPx(): Promise<number> {
  const isMac = platform() === 'macos';
  const isMaximized = await getCurrentWindow().isMaximized();

  return isMac && !isMaximized ? 30 : 0;
}

interface Args {
  app: InstalledSageApp | null | undefined;
  containerRef: React.RefObject<HTMLDivElement | null>;
  requestApproval: unknown;
}

export function useAppEmbeddedRuntime({ app, containerRef }: Args) {
  const syncBounds = useCallback(
    async (installedAppId: string) => {
      const webview = await getRuntimeWebview(installedAppId);
      const container = containerRef.current;

      if (!webview || !container) {
        return;
      }

      const rect = container.getBoundingClientRect();
      const width = Math.max(1, Math.round(rect.width));
      const height = Math.max(
        1,
        Math.round(rect.height - (await getMacWindowedTopInsetPx())),
      );
      const x = Math.round(rect.left);
      const y = Math.round(rect.top + (await getMacWindowedTopInsetPx()));

      await webview.setPosition(new LogicalPosition(x, y));
      await webview.setSize(new LogicalSize(width, height));
    },
    [containerRef],
  );

  const scheduleSyncBounds = useCallback(
    (installedAppId: string) => {
      requestAnimationFrame(() => {
        void syncBounds(installedAppId).catch((err) => {
          const message = err instanceof Error ? err.message : String(err);

          if (message.includes('webview not found')) {
            return;
          }

          console.error('Failed to sync embedded app webview bounds:', err);
        });
      });
    },
    [syncBounds],
  );

  useEffect(() => {
    if (!app || !containerRef.current) {
      return;
    }

    const installedApp = app;

    let disposed = false;
    let resizeObserver: ResizeObserver | null = null;
    let removeWindowResize: (() => void) | null = null;
    let delayedSyncTimers: number[] = [];

    const clearDelayedSyncTimers = () => {
      delayedSyncTimers.forEach((id) => {
        window.clearTimeout(id);
      });
      delayedSyncTimers = [];
    };

    const mount = async () => {
      await ensureInlineRuntime(installedApp);
      await markRuntimeVisible(installedApp.id, true);

      scheduleSyncBounds(installedApp.id);

      delayedSyncTimers = [0, 50, 150, 300].map((delay) =>
        window.setTimeout(() => {
          if (!disposed) {
            scheduleSyncBounds(installedApp.id);
          }
        }, delay),
      );

      resizeObserver = new ResizeObserver(() => {
        if (!disposed) {
          scheduleSyncBounds(installedApp.id);
        }
      });

      const container = containerRef.current;
      if (!container) {
        return;
      }

      resizeObserver.observe(container);

      const handleWindowResize = () => {
        if (!disposed) {
          scheduleSyncBounds(installedApp.id);
        }
      };

      window.addEventListener('resize', handleWindowResize);
      removeWindowResize = () => {
        window.removeEventListener('resize', handleWindowResize);
      };
    };

    void mount().catch((err) => {
      console.error('Failed to attach app runtime:', err);
    });

    return () => {
      disposed = true;
      void markRuntimeVisible(installedApp.id, false);
      resizeObserver?.disconnect();
      removeWindowResize?.();
      clearDelayedSyncTimers();
    };
  }, [app, containerRef, scheduleSyncBounds]);

  return {
    scheduleSyncBounds,
  };
}

