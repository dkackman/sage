import { useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import {
  isBridgeRequest,
  type BridgeApprovalRequest,
  type SageBridgeEventPayload,
  type SageBridgeResponse,
} from '@/lib/apps/bridge';
import { acceptSandboxBridgeSend } from '@/lib/apps/sandboxRuntimeStore';
import type {
  ResolveBridgeApprovalArgs,
  RustBridgeHandleResult,
  RustBridgeRequest,
  RustBridgeResponse,
} from '@/bindings.ts';

interface Args {
  requestApproval: (
    request: BridgeApprovalRequest,
  ) => Promise<{ approved: boolean; reason?: string }>;
}

interface RustSandboxBridgeSendEvent {
  appId: string;
  payloadJson: string;
}

type RustBridgeSuccessLike = Extract<
  RustBridgeResponse,
  { resultJson: string }
>;

function isRustBridgeSuccess(
  response: RustBridgeResponse,
): response is RustBridgeSuccessLike {
  return 'resultJson' in response;
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
  if (isRustBridgeSuccess(response)) {
    return {
      channel: 'sage-bridge',
      bridgeVersion: 'v1',
      id: response.id,
      ok: true,
      result: parseJsonOrNull(response.resultJson),
    };
  }

  return {
    channel: 'sage-bridge',
    bridgeVersion: 'v1',
    id: response.id,
    ok: false,
    error: response.error,
  };
}

export function useBridgeHost({ requestApproval }: Args) {
  const hostWebview = getCurrentWebview();

  useEffect(() => {
    let unlistenRequest: (() => void) | null = null;
    let unlistenSandbox: (() => void) | null = null;

    const mount = async () => {
      unlistenRequest = await hostWebview.listen<SageBridgeEventPayload>(
        'sage-bridge:request',
        ({ payload }) => {
          if (!payload || !isBridgeRequest(payload.request)) {
            return;
          }

          const sourceLabel = payload.sourceLabel;

          const run = async () => {
            const rustRequest: RustBridgeRequest = {
              channel: payload.request.channel,
              bridgeVersion: payload.request.bridgeVersion ?? null,
              id: payload.request.id,
              method: payload.request.method,
              paramsJson:
                payload.request.params === undefined
                  ? null
                  : JSON.stringify(payload.request.params),
            };

            const result = await invoke<RustBridgeHandleResult>(
              'apps_handle_bridge_request',
              {
                sourceLabel,
                request: rustRequest,
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

            const approvalResult = await requestApproval({
              kind: 'send_xch',
              app: result.approval.app,
              sourceLabel: result.approval.sourceLabel,
              requestId: result.approval.requestId,
              params: parseJsonOrNull(
                result.approval.paramsJson,
              ) as BridgeApprovalRequest['params'],
            });

            const approvalArgs: ResolveBridgeApprovalArgs = {
              approvalId: result.approvalId,
              approved: approvalResult.approved,
              reason: approvalResult.reason ?? null,
            };

            const rawResponse = await invoke<RustBridgeResponse>(
              'apps_resolve_bridge_approval',
              {
                args: approvalArgs,
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

      unlistenSandbox = await hostWebview.listen<RustSandboxBridgeSendEvent>(
        'sage-sandbox:report',
        ({ payload }) => {
          if (!payload) {
            return;
          }

          acceptSandboxBridgeSend({
            appId: payload.appId,
            payload: parseJsonOrNull(payload.payloadJson) as never,
          });
        },
      );
    };

    void mount().catch((err) => {
      console.error('Failed to mount bridge host listener:', err);
    });

    return () => {
      unlistenRequest?.();
      unlistenSandbox?.();
    };
  }, [hostWebview, requestApproval]);
}
