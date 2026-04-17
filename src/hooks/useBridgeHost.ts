import { useEffect } from 'react';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import {
  handleBridgeRequest,
  isBridgeRequest,
  type SageBridgeEventPayload,
} from '@/lib/apps/bridge';
import { getBuiltinApp } from '@/lib/apps/registry';
import type { InstalledSageApp } from '@/bindings';

interface Args {
  requestApproval: Parameters<typeof handleBridgeRequest>[2]['requestApproval'];
  getApp: (appId: string) => InstalledSageApp | undefined;
}

export function useBridgeHost({ requestApproval, getApp }: Args) {
  const hostWebview = getCurrentWebview();

  useEffect(() => {
    let unlisten: (() => void) | null = null;

    const mount = async () => {
      unlisten = await hostWebview.listen<SageBridgeEventPayload>(
        'sage-bridge:request',
        ({ payload }) => {
          if (!payload || !isBridgeRequest(payload.request)) {
            return;
          }

          const sourceLabel = payload.sourceLabel;
          const prefix = 'app-inline-';

          if (!sourceLabel.startsWith(prefix)) {
            return;
          }

          const appId = sourceLabel.slice(prefix.length);

          const run = async () => {
            const app = getApp(appId) ?? (await getBuiltinApp(appId));

            if (!app) {
              return;
            }

            const response = await handleBridgeRequest(
              {
                app,
                sourceLabel,
              },
              payload.request,
              {
                requestApproval,
              },
            );

            await hostWebview.emitTo(
              sourceLabel,
              'sage-bridge:response',
              response,
            );
          };

          void run().catch((err) => {
            console.error('Failed to handle bridge request:', err);
          });
        },
      );
    };

    void mount().catch((err) => {
      console.error('Failed to mount bridge host listener:', err);
    });

    return () => {
      unlisten?.();
    };
  }, [hostWebview, getApp, requestApproval]);
}
