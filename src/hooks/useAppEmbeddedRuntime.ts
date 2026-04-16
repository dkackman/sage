import React, { useCallback, useEffect } from 'react';
import { LogicalPosition, LogicalSize } from '@tauri-apps/api/dpi';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import {
  handleBridgeRequest,
  isBridgeRequest,
  type SageBridgeEventPayload,
} from '@/lib/apps/bridge';
import {
  ensureInlineRuntime,
  getRuntimeWebview,
  markRuntimeVisible,
} from '@/lib/apps/runtimeRegistry';
import type { InstalledSageApp } from '@/bindings';

interface Args {
  app: InstalledSageApp | null | undefined;
  containerRef: React.RefObject<HTMLDivElement | null>;
  requestApproval: Parameters<typeof handleBridgeRequest>[2]['requestApproval'];
}

export function useAppEmbeddedRuntime({
  app,
  containerRef,
  requestApproval,
}: Args) {
  const syncBounds = useCallback(
    async (installedAppId: string) => {
      const webview = await getRuntimeWebview(installedAppId);
      const container = containerRef.current;

      if (!webview || !container) {
        return;
      }

      const rect = container.getBoundingClientRect();
      const width = Math.max(1, Math.round(rect.width));
      const height = Math.max(1, Math.round(rect.height));
      const x = Math.round(rect.left);
      const y = Math.round(rect.top);

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
    const hostWebview = getCurrentWebview();

    let disposed = false;
    let resizeObserver: ResizeObserver | null = null;
    let unlistenBridge: (() => void) | null = null;
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

      const expectedSourceLabel = `app-inline-${installedApp.id}`;

      unlistenBridge = await hostWebview.listen<SageBridgeEventPayload>(
        'sage-bridge:request',
        ({ payload }) => {
          if (
            !payload ||
            payload.sourceLabel !== expectedSourceLabel ||
            !isBridgeRequest(payload.request)
          ) {
            return;
          }

          void handleBridgeRequest(
            {
              app: installedApp,
              sourceLabel: payload.sourceLabel,
            },
            payload.request,
            {
              requestApproval,
            },
          ).then((response) => {
            void hostWebview.emitTo(
              payload.sourceLabel,
              'sage-bridge:response',
              response,
            );
          });
        },
      );
    };

    void mount().catch((err) => {
      console.error('Failed to attach app runtime:', err);
    });

    return () => {
      disposed = true;
      void markRuntimeVisible(installedApp.id, false);
      resizeObserver?.disconnect();
      unlistenBridge?.();
      removeWindowResize?.();
      clearDelayedSyncTimers();
    };
  }, [app, containerRef, requestApproval, scheduleSyncBounds]);

  return {
    scheduleSyncBounds,
  };
}
