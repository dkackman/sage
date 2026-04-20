import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import type { BridgeApprovalRequest } from '@/lib/apps/bridge';

interface Args {
  requestApproval: (
    request: BridgeApprovalRequest,
  ) => Promise<{ approved: boolean; reason?: string }>;
}

interface RustBridgeApprovalRequestPayload {
  kind: string;
  app: BridgeApprovalRequest['app'];
  sourceLabel: string;
  requestId: string;
  paramsJson: string;
}

interface RustBridgeApprovalEvent {
  approvalId: string;
  approval: RustBridgeApprovalRequestPayload;
}

function parseJsonOrNull(value: string | null | undefined): unknown {
  if (value == null) {
    return null;
  }

  try {
    return JSON.parse(value);
  } catch (err) {
    console.error('Failed to parse Rust bridge approval payload:', err, value);
    return null;
  }
}

function toBridgeApprovalRequest(
  payload: RustBridgeApprovalRequestPayload,
): BridgeApprovalRequest | null {
  if (payload.kind === 'send_xch') {
    return {
      kind: 'send_xch',
      app: payload.app,
      sourceLabel: payload.sourceLabel,
      requestId: payload.requestId,
      params: parseJsonOrNull(
        payload.paramsJson,
      ) as BridgeApprovalRequest extends {
        kind: 'send_xch';
        params: infer P;
      }
        ? P
        : never,
    };
  }

  if (payload.kind === 'capability_grant') {
    const parsed = parseJsonOrNull(payload.paramsJson) as {
      capability?: string;
    } | null;
    if (!parsed?.capability) {
      return null;
    }

    return {
      kind: 'capability_grant',
      app: payload.app,
      sourceLabel: payload.sourceLabel,
      requestId: payload.requestId,
      capability: parsed.capability,
    };
  }

  if (payload.kind === 'network_whitelist_grant') {
    const parsed = parseJsonOrNull(payload.paramsJson) as {
      entry?: { scheme?: string; host?: string };
    } | null;

    if (!parsed?.entry?.scheme || !parsed?.entry?.host) {
      return null;
    }

    return {
      kind: 'network_whitelist_grant',
      app: payload.app,
      sourceLabel: payload.sourceLabel,
      requestId: payload.requestId,
      entry: {
        scheme: parsed.entry.scheme,
        host: parsed.entry.host,
      },
    };
  }

  return null;
}

export function useBridgeHost({ requestApproval }: Args) {
  const [isReady, setIsReady] = useState(false);
  const requestApprovalRef = useRef(requestApproval);

  useEffect(() => {
    requestApprovalRef.current = requestApproval;
  }, [requestApproval]);

  useEffect(() => {
    let disposed = false;
    let shouldUnlistenWhenReady = false;
    let unlistenRequest: (() => void) | null = null;

    const currentWindow = getCurrentWindow();

    const mount = async () => {
      const unlisten = await currentWindow.listen<RustBridgeApprovalEvent>(
        'apps:bridge-approval-requested',
        ({ payload }) => {
          if (!payload?.approvalId || !payload.approval) {
            return;
          }

          const approvalRequest = toBridgeApprovalRequest(payload.approval);
          if (!approvalRequest) {
            console.error('Failed to decode bridge approval request:', payload);
            return;
          }

          const run = async () => {
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
        shouldUnlistenWhenReady = false;
        unlisten();
        return;
      }

      if (shouldUnlistenWhenReady) {
        shouldUnlistenWhenReady = false;
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

      if (unlistenRequest) {
        unlistenRequest();
        unlistenRequest = null;
      } else {
        shouldUnlistenWhenReady = true;
      }
    };
  }, []);

  return { isReady };
}
