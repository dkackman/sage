import { useEffect } from 'react';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import {
  handleBridgeRequest,
  isBridgeRequest,
  type SageBridgeEventPayload,
} from '@/lib/apps/bridge';
import { getBuiltinApp } from '@/lib/apps/registry';
import type { InstalledSageApp } from '@/bindings';
import { invoke } from '@tauri-apps/api/core';

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

          const claimedAppId = sourceLabel.slice(prefix.length);

          const run = async () => {
            const confirmedAppId = await invoke<string>(
              'apps_assert_bridge_origin',
              {
                sourceLabel,
              },
            );

            if (confirmedAppId !== claimedAppId) {
              throw new Error(
                `Bridge origin mismatch for ${sourceLabel}: claimed ${claimedAppId}, confirmed ${confirmedAppId}`,
              );
            }

            const app =
              getApp(confirmedAppId) ?? (await getBuiltinApp(confirmedAppId));

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
      )
    };

    void mount().catch((err) => {
      console.error('Failed to mount bridge host listener:', err);
    });

    return () => {
      unlisten?.();
    };
  }, [hostWebview, getApp, requestApproval]);
}
