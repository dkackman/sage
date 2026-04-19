import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import {
  isBridgeRequest,
  type BridgeApprovalRequest,
  type SageBridgeEventPayload,
  type SageBridgeResponse,
} from '@/lib/apps/bridge';
import { RustBridgeHandleResult, RustBridgeResponse } from '@/bindings.ts';

interface Args {
  requestApproval: (
    request: BridgeApprovalRequest,
  ) => Promise<{ approved: boolean; reason?: string }>;
}

function parseJsonOrNull(value: string | null | undefined): unknown {
  if (value == null) {
    return null;
  }

  try {
    return JSON.parse(value);
  } catch (err) {
    console.error('Failed to parse JSON payload from Rust bridge:', err, value);
    return null;
  }
}

function toSdkBridgeResponse(response: RustBridgeResponse): SageBridgeResponse {
  if ('resultJson' in response) {
    return {
      channel: 'sage-bridge',
      bridgeVersion: 'v1',
      id: response.id,
      ok: true,
      result: parseJsonOrNull(response.resultJson),
    };
  }

  if ('error' in response) {
    return {
      channel: 'sage-bridge',
      bridgeVersion: 'v1',
      id: response.id,
      ok: false,
      error: response.error,
    };
  }

  return {
    channel: 'sage-bridge',
    bridgeVersion: 'v1',
    id: 'unknown',
    ok: false,
    error: {
      code: 'internal_error',
      message: 'Unknown Rust bridge response shape',
    },
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
    let shouldUnlistenWhenReady = false;
    let unlistenRequest: (() => void) | null = null;

    const hostWebview = getCurrentWebview();

    const mount = async () => {
      const unlisten = await hostWebview.listen<SageBridgeEventPayload>(
        'sage-bridge:request',
        ({ payload }) => {
          if (!payload || !isBridgeRequest(payload.request)) {
            return;
          }

          const sourceLabel = payload.sourceLabel;

          const run = async () => {
            const result = await invoke<RustBridgeHandleResult>(
              'apps_handle_bridge_request',
              {
                sourceLabel,
                request: {
                  channel: payload.request.channel,
                  bridgeVersion: payload.request.bridgeVersion ?? null,
                  id: payload.request.id,
                  method: payload.request.method,
                  paramsJson:
                    payload.request.params === undefined
                      ? null
                      : JSON.stringify(payload.request.params),
                },
              },
            );

            if (result.kind === 'immediate') {
              await hostWebview.emitTo(
                sourceLabel,
                'sage-bridge:response',
                toSdkBridgeResponse(result.response),
              );
              return;
            }

            const approvalResult = await requestApprovalRef.current({
              kind: 'send_xch',
              app: result.approval.app,
              sourceLabel: result.approval.sourceLabel,
              requestId: result.approval.requestId,
              params: parseJsonOrNull(
                result.approval.paramsJson,
              ) as BridgeApprovalRequest['params'],
            });

            const rawResponse = await invoke<RustBridgeResponse>(
              'apps_resolve_bridge_approval',
              {
                args: {
                  approvalId: result.approvalId,
                  approved: approvalResult.approved,
                  reason: approvalResult.reason ?? null,
                },
              },
            );

            await hostWebview.emitTo(
              sourceLabel,
              'sage-bridge:response',
              toSdkBridgeResponse(rawResponse),
            );
          };

          void run().catch((err) => {
            console.error('Failed to handle bridge request:', err);
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
      console.error('Failed to mount bridge host listener:', err);
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
