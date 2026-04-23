import { invoke } from '@tauri-apps/api/core';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import type {
  AppGetInfoResult,
  BridgePingResult,
  BridgeSendResult,
  GrantedCapabilitiesChangeEvent,
  GrantedNetworkWhitelistChangeEvent,
  ReadyToStopParams,
  RequestCapabilityGrantParams,
  RequestCapabilityGrantResult,
  RequestNetworkWhitelistGrantParams,
  RequestNetworkWhitelistGrantResult,
  RuntimeAckResult,
  SageBridgeRequest,
  SageBridgeResponse,
  SageBridgeRuntimeEvent,
  SageBridgeSendPayload,
  SageBridgeVersion,
  SageClient,
  SageLifecycleBeforeStopDetail,
  SetBeforeStopListenerParams,
  TransactionResponse,
  WalletSendXchParams,
} from './types';

export const SAGE_BRIDGE_VERSION: SageBridgeVersion = 'v1';

type SageListenEvent<T = unknown> = {
  payload: T;
};

type SageUnlisten = () => void;

type SageWebviewHandle = {
  label: string;
  listen<T = unknown>(
    event: string,
    handler: (event: SageListenEvent<T>) => void,
  ): Promise<SageUnlisten>;
};

type SageWindow = Window &
  typeof globalThis & {
    __SAGE__?: SageClient;
    __SAGE_APP_INFO__?: AppGetInfoResult;
    __SAGE_RUNTIME_BRIDGE_INITIALIZED__?: boolean;
  };

type PendingBridgeRequest = {
  resolve: (value: unknown) => void;
  reject: (reason?: unknown) => void;
  timeoutId: number;
  method: string;
};

type RustBridgeResponse =
  | {
      channel: string;
      bridgeVersion: string;
      id: string;
      ok: true;
      resultJson: string;
    }
  | {
      channel: string;
      bridgeVersion: string;
      id: string;
      ok: false;
      error: {
        code: string;
        message: string;
      };
    };

type RustBridgeInvokeResult =
  | {
      kind: 'immediate';
      response: RustBridgeResponse;
    }
  | {
      kind: 'pending';
    };

type BeforeStopPublicDetail = Omit<SageLifecycleBeforeStopDetail, 'requestId'>;

function getSageWindow(): SageWindow {
  return window as SageWindow;
}

function buildFallbackAppInfo(): AppGetInfoResult {
  return {
    id: 'unknown',
    name: 'Unknown App',
    version: '0.0.0',
    requestedPermissions: {
      network: {
        whitelist: {
          required: [],
          optional: [],
        },
      },
      capabilities: {
        required: [],
        optional: [],
      },
    },
    capabilities: [],
    network: [],
  };
}

function tryGetCurrentWebview(): SageWebviewHandle | null {
  try {
    return getCurrentWebview() as SageWebviewHandle;
  } catch {
    return null;
  }
}

function isObject(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object';
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

  return {
    channel: 'sage-bridge',
    bridgeVersion: 'v1',
    id: response.id,
    ok: false,
    error: response.error,
  };
}

function isBridgeRuntimeEvent(value: unknown): value is SageBridgeRuntimeEvent {
  if (!isObject(value)) {
    return false;
  }

  if (value.channel !== 'sage-bridge') {
    return false;
  }

  return (
    value.type === 'grantedCapabilitiesChange' ||
    value.type === 'grantedNetworkWhitelistChange'
  );
}

function isGrantedCapabilitiesChangeEvent(
  value: unknown,
): value is GrantedCapabilitiesChangeEvent {
  return (
    isObject(value) &&
    value.channel === 'sage-bridge' &&
    value.type === 'grantedCapabilitiesChange'
  );
}

function isGrantedNetworkWhitelistChangeEvent(
  value: unknown,
): value is GrantedNetworkWhitelistChangeEvent {
  return (
    isObject(value) &&
    value.channel === 'sage-bridge' &&
    value.type === 'grantedNetworkWhitelistChange'
  );
}

export function initSageRuntimeBridge(): boolean {
  const w = getSageWindow();

  if (w.__SAGE__) {
    return true;
  }

  if (w.__SAGE_RUNTIME_BRIDGE_INITIALIZED__) {
    return true;
  }

  const maybeWebview = tryGetCurrentWebview();
  if (!maybeWebview) {
    return false;
  }

  const webview: SageWebviewHandle = maybeWebview;
  w.__SAGE_RUNTIME_BRIDGE_INITIALIZED__ = true;

  const pendingRequests = new Map<string, PendingBridgeRequest>();
  const beforeStopHandlers = new Set<
    (detail: BeforeStopPublicDetail) => void | Promise<void>
  >();
  let beforeStopRegistered = false;

  function rejectAllPending(reason: string) {
    for (const [id, pending] of pendingRequests.entries()) {
      window.clearTimeout(pending.timeoutId);
      pending.reject(new Error(reason));
      pendingRequests.delete(id);
    }
  }

  async function callHost<T>(method: string, params?: unknown): Promise<T> {
    const id = `sage-${Date.now()}-${Math.random().toString(36).slice(2)}`;

    return await new Promise<T>((resolve, reject) => {
      const timeoutId = window.setTimeout(() => {
        const pending = pendingRequests.get(id);
        if (!pending) {
          return;
        }

        pendingRequests.delete(id);
        reject(new Error(`Sage bridge timeout for ${method}`));
      }, 30000);

      pendingRequests.set(id, {
        resolve: (value) => resolve(value as T),
        reject,
        timeoutId,
        method,
      });

      void (async () => {
        try {
          const request: SageBridgeRequest = {
            channel: 'sage-bridge',
            bridgeVersion: SAGE_BRIDGE_VERSION,
            id,
            method,
            params,
          };

          const result = await invoke<RustBridgeInvokeResult>(
            'apps_invoke_bridge',
            {
              request: {
                channel: request.channel,
                bridgeVersion: request.bridgeVersion ?? null,
                id: request.id,
                method: request.method,
                paramsJson:
                  request.params === undefined
                    ? null
                    : JSON.stringify(request.params),
              },
            },
          );

          if (result.kind === 'immediate') {
            const response = toSdkBridgeResponse(result.response);
            pendingRequests.delete(id);
            window.clearTimeout(timeoutId);

            if (response.ok) {
              resolve(response.result as T);
            } else {
              reject(new Error(response.error.message));
            }
          }
        } catch (error: unknown) {
          const pending = pendingRequests.get(id);
          if (!pending) {
            return;
          }

          pendingRequests.delete(id);
          window.clearTimeout(timeoutId);
          reject(error instanceof Error ? error : new Error(String(error)));
        }
      })();
    });
  }

  async function syncBeforeStopRegistration() {
    const shouldBeRegistered = beforeStopHandlers.size > 0;
    if (beforeStopRegistered === shouldBeRegistered) {
      return;
    }

    beforeStopRegistered = shouldBeRegistered;

    try {
      await callHost<RuntimeAckResult>('app.lifecycle.setBeforeStopListener', {
        active: shouldBeRegistered,
      } satisfies SetBeforeStopListenerParams);
    } catch (error) {
      console.error('Failed to sync before-stop listener registration:', error);
    }
  }

  webview
    .listen('sage-bridge:event', (event: SageListenEvent) => {
      const data = event.payload;

      if (!isBridgeRuntimeEvent(data)) {
        return;
      }

      try {
        if (isGrantedCapabilitiesChangeEvent(data)) {
          window.dispatchEvent(
            new CustomEvent<GrantedCapabilitiesChangeEvent>(
              'sage:granted-capabilities-change',
              { detail: data },
            ),
          );
          return;
        }

        if (isGrantedNetworkWhitelistChangeEvent(data)) {
          window.dispatchEvent(
            new CustomEvent<GrantedNetworkWhitelistChangeEvent>(
              'sage:granted-network-whitelist-change',
              { detail: data },
            ),
          );
        }
      } catch (error: unknown) {
        console.error('Failed to dispatch Sage bridge runtime event:', error);
      }
    })
    .catch((error: unknown) => {
      console.error('Failed to subscribe to sage-bridge:event:', error);
    });

  webview
    .listen(
      'sage-bridge:response',
      (event: SageListenEvent<SageBridgeResponse>) => {
        const data = event.payload;

        if (
          !data ||
          data.channel !== 'sage-bridge' ||
          data.bridgeVersion !== SAGE_BRIDGE_VERSION
        ) {
          return;
        }

        const pending = pendingRequests.get(data.id);
        if (!pending) {
          return;
        }

        pendingRequests.delete(data.id);
        window.clearTimeout(pending.timeoutId);

        if (data.ok) {
          pending.resolve(data.result as unknown);
        } else {
          pending.reject(
            new Error(data.error?.message || 'Unknown Sage bridge error'),
          );
        }
      },
    )
    .catch((error: unknown) => {
      console.error('Failed to subscribe to sage-bridge:response:', error);
    });

  webview
    .listen<SageLifecycleBeforeStopDetail>(
      'sage-lifecycle:before-stop',
      (event: SageListenEvent<SageLifecycleBeforeStopDetail>) => {
        const detail = event.payload;
        rejectAllPending('Sage runtime is stopping');

        if (!detail?.requestId || beforeStopHandlers.size === 0) {
          return;
        }

        const publicDetail: BeforeStopPublicDetail = {
          reason: detail.reason,
          appId: detail.appId,
          runtimeId: detail.runtimeId,
        };

        const handlers = Array.from(beforeStopHandlers);

        void Promise.allSettled(
          handlers.map((handler) => Promise.resolve(handler(publicDetail))),
        ).finally(() => {
          void callHost<RuntimeAckResult>('app.lifecycle.readyToStop', {
            requestId: detail.requestId,
          } satisfies ReadyToStopParams).catch((error: unknown) => {
            console.error('Failed to acknowledge before-stop:', error);
          });
        });
      },
    )
    .catch((error: unknown) => {
      console.error(
        'Failed to subscribe to sage-lifecycle:before-stop:',
        error,
      );
    });

  w.__SAGE__ = {
    initialAppInfo: w.__SAGE_APP_INFO__ ?? buildFallbackAppInfo(),
    app: {
      async bridgePing() {
        return await callHost<BridgePingResult>('bridge.ping');
      },

      async bridgeSend(input: SageBridgeSendPayload) {
        return await callHost<BridgeSendResult>('bridge.send', input);
      },

      async getInfo() {
        return await callHost<AppGetInfoResult>('app.getInfo');
      },

      async getCapabilities() {
        return await callHost<string[]>('sage.getCapabilities');
      },

      async requestCapabilityGrant(input: RequestCapabilityGrantParams) {
        return await callHost<RequestCapabilityGrantResult>(
          'sage.requestCapabilityGrant',
          input,
        );
      },

      async requestNetworkWhitelistGrant(
        input: RequestNetworkWhitelistGrantParams,
      ) {
        return await callHost<RequestNetworkWhitelistGrantResult>(
          'sage.requestNetworkWhitelistGrant',
          input,
        );
      },

      onGrantedCapabilitiesChange(handler) {
        const listener = (event: Event) => {
          const custom = event as CustomEvent<GrantedCapabilitiesChangeEvent>;
          handler(custom.detail);
        };

        window.addEventListener(
          'sage:granted-capabilities-change',
          listener as EventListener,
        );

        return () => {
          window.removeEventListener(
            'sage:granted-capabilities-change',
            listener as EventListener,
          );
        };
      },

      onGrantedNetworkWhitelistChange(handler) {
        const listener = (event: Event) => {
          const custom =
            event as CustomEvent<GrantedNetworkWhitelistChangeEvent>;
          handler(custom.detail);
        };

        window.addEventListener(
          'sage:granted-network-whitelist-change',
          listener as EventListener,
        );

        return () => {
          window.removeEventListener(
            'sage:granted-network-whitelist-change',
            listener as EventListener,
          );
        };
      },

      lifecycle: {
        onBeforeStop(handler) {
          beforeStopHandlers.add(handler);
          void syncBeforeStopRegistration();

          return () => {
            beforeStopHandlers.delete(handler);
            void syncBeforeStopRegistration();
          };
        },
      },
    },

    wallet: {
      async sendXch(input: WalletSendXchParams) {
        return await callHost<TransactionResponse>('wallet.sendXch', input);
      },
    },
  };

  return true;
}
