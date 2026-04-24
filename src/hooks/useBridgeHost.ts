import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import type { RustBridgeApprovalEvent } from '@/bindings';
import type { BridgeApprovalRequest } from '@/lib/apps/bridge/types.ts';

interface Args {
  requestApproval: (
    request: BridgeApprovalRequest,
  ) => Promise<{ approved: boolean; reason?: string }>;
}

function toBridgeApprovalRequest(
  event: RustBridgeApprovalEvent,
): BridgeApprovalRequest {
  return {
    ...event.approval,
    approvalId: event.approvalId,
  };
}

export function useBridgeHost({ requestApproval }: Args) {
  const [isReady, setIsReady] = useState(false);
  const requestApprovalRef = useRef(requestApproval);

  useEffect(() => {
    requestApprovalRef.current = requestApproval;
  }, [requestApproval]);

  useEffect(() => {
    let disposed = false;
    let unlistenRequest: (() => void) | null = null;

    const mount = async () => {
      const unlisten = await getCurrentWindow().listen<RustBridgeApprovalEvent>(
        'apps:bridge-approval-requested',
        ({ payload }) => {
          const run = async () => {
            const approvalRequest = toBridgeApprovalRequest(payload);
            const approvalResult =
              await requestApprovalRef.current(approvalRequest);

            await invoke('apps_resolve_bridge_approval', {
              args: {
                approvalId: payload.approvalId,
                approved: approvalResult.approved,
                reason: approvalResult.reason ?? null,
              },
            });
          };

          void run().catch((err) => {
            console.error('Failed to resolve bridge approval:', err);
          });
        },
      );

      if (disposed) {
        unlisten();
        return;
      }

      unlistenRequest = unlisten;
      setIsReady(true);
    };

    void mount().catch((err) => {
      console.error('Failed to mount bridge approval listener:', err);
    });

    return () => {
      disposed = true;
      setIsReady(false);
      unlistenRequest?.();
    };
  }, []);

  return { isReady };
}
