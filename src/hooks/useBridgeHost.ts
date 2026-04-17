import { useEffect } from 'react';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import {
  handleBridgeRequest,
  isBridgeRequest,
  type SageBridgeEventPayload,
} from '@/lib/apps/bridge';
import { useApps } from '@/contexts/AppsContext';
import { getBuiltinApp } from '@/lib/apps/registry';

interface Args {
  requestApproval: Parameters<typeof handleBridgeRequest>[2]['requestApproval'];
}
export function useBridgeHost({ requestApproval }: Args) {
  const hostWebview = getCurrentWebview();
  const { getApp } = useApps();

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
          const installedApp = getApp(appId);

          const run = async () => {
            const app = installedApp ?? (await getBuiltinApp(appId));

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
