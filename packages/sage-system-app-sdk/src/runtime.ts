import { invoke } from '@tauri-apps/api/core';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import type {
  RuntimeTargetParams,
  SageAppRuntimeRecord,
  SageSystemBridgeRequest,
  SageSystemBridgeResponse,
  SageSystemClient,
  SageSystemBridgeVersion,
  SystemKillRuntimeResult,
} from './types';

export const SAGE_SYSTEM_BRIDGE_CHANNEL = 'sage-system-bridge';
export const SAGE_SYSTEM_BRIDGE_VERSION: SageSystemBridgeVersion = 'v1';

type SageSystemListenEvent<T = unknown> = {
  payload: T;
};

type SageUnlisten = () => void;

type SageWebviewHandle = {
  label: string;
  listen<T = unknown>(
    event: string,
    handler: (event: SageSystemListenEvent<T>) => void,
  ): Promise<SageUnlisten>;
};

type SageSystemWindow = Window &
  typeof globalThis & {
    __SAGE_SYSTEM__?: SageSystemClient;
    __SAGE_SYSTEM_RUNTIME_BRIDGE_INITIALIZED__?: boolean;
  };

type PendingSystemBridgeRequest = {
  resolve: (value: unknown) => void;
  reject: (reason?: unknown) => void;
  timeoutId: number;
  method: string;
};

type RustSystemBridgeResponse =
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

type RustSystemBridgeInvokeResult =
  | {
      kind: 'immediate';
      response: RustSystemBridgeResponse;
    }
  | {
      kind: 'pending';
    };

function getSageWindow(): SageSystemWindow {
  return window as SageSystemWindow;
}

function tryGetCurrentWebview(): SageWebviewHandle | null {
  try {
    return getCurrentWebview() as SageWebviewHandle;
  } catch {
    return null;
  }
}

function parseJsonOrNull(value: string | null | undefined): unknown {
  if (value == null) {
    return null;
  }

  try {
    return JSON.parse(value);
  } catch (err) {
    console.error(
      'Failed to parse JSON payload from Rust system bridge:',
      err,
      value,
    );
    return null;
  }
}

function toSdkSystemBridgeResponse(
  response: RustSystemBridgeResponse,
): SageSystemBridgeResponse {
  if ('resultJson' in response) {
    return {
      channel: SAGE_SYSTEM_BRIDGE_CHANNEL,
      bridgeVersion: SAGE_SYSTEM_BRIDGE_VERSION,
      id: response.id,
      ok: true,
      result: parseJsonOrNull(response.resultJson),
    };
  }

  return {
    channel: SAGE_SYSTEM_BRIDGE_CHANNEL,
    bridgeVersion: SAGE_SYSTEM_BRIDGE_VERSION,
    id: response.id,
    ok: false,
    error: response.error,
  };
}

export function initSageSystemRuntimeBridge(): boolean {
  const w = getSageWindow();

  if (w.__SAGE_SYSTEM__) {
    return true;
  }

  if (w.__SAGE_SYSTEM_RUNTIME_BRIDGE_INITIALIZED__) {
    return true;
  }

  const maybeWebview = tryGetCurrentWebview();
  if (!maybeWebview) {
    return false;
  }

  const webview: SageWebviewHandle = maybeWebview;
  w.__SAGE_SYSTEM_RUNTIME_BRIDGE_INITIALIZED__ = true;

  const pendingRequests = new Map<string, PendingSystemBridgeRequest>();

  function rejectAllPending(reason: string) {
    for (const [id, pending] of pendingRequests.entries()) {
      window.clearTimeout(pending.timeoutId);
      pending.reject(new Error(reason));
      pendingRequests.delete(id);
    }
  }

  webview
    .listen(
      `${SAGE_SYSTEM_BRIDGE_CHANNEL}:response`,
      (event: SageSystemListenEvent<SageSystemBridgeResponse>) => {
        const data = event.payload;

        if (
          !data ||
          data.channel !== SAGE_SYSTEM_BRIDGE_CHANNEL ||
          data.bridgeVersion !== SAGE_SYSTEM_BRIDGE_VERSION
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
            new Error(
              data.error?.message || 'Unknown Sage system bridge error',
            ),
          );
        }
      },
    )
    .catch((error: unknown) => {
      console.error(
        `Failed to subscribe to ${SAGE_SYSTEM_BRIDGE_CHANNEL}:response:`,
        error,
      );
    });

  webview
    .listen('sage-lifecycle:before-stop', () => {
      rejectAllPending('Sage system runtime is stopping');
    })
    .catch((error: unknown) => {
      console.error(
        'Failed to subscribe to sage-lifecycle:before-stop:',
        error,
      );
    });

  async function callHost<T>(method: string, params?: unknown): Promise<T> {
    const id = `sage-system-${Date.now()}-${Math.random().toString(36).slice(2)}`;

    return await new Promise<T>((resolve, reject) => {
      const timeoutId = window.setTimeout(() => {
        const pending = pendingRequests.get(id);
        if (!pending) {
          return;
        }

        pendingRequests.delete(id);
        reject(new Error(`Sage system bridge timeout for ${method}`));
      }, 30000);

      pendingRequests.set(id, {
        resolve: (value) => resolve(value as T),
        reject,
        timeoutId,
        method,
      });

      void (async () => {
        try {
          const request: SageSystemBridgeRequest = {
            channel: SAGE_SYSTEM_BRIDGE_CHANNEL,
            bridgeVersion: SAGE_SYSTEM_BRIDGE_VERSION,
            id,
            method,
            params,
          };

          const result = await invoke<RustSystemBridgeInvokeResult>(
            'apps_invoke_system_bridge',
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
            const response = toSdkSystemBridgeResponse(result.response);
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

  w.__SAGE_SYSTEM__ = {
    runtimeManager: {
      async listRuntimes() {
        return await callHost<SageAppRuntimeRecord[]>('system.listRuntimes');
      },

      async focusRuntime(input: RuntimeTargetParams) {
        return await callHost<SageAppRuntimeRecord>(
          'system.focusRuntime',
          input,
        );
      },

      async hideRuntime(input: RuntimeTargetParams) {
        return await callHost<SageAppRuntimeRecord>(
          'system.hideRuntime',
          input,
        );
      },

      async killRuntime(input: RuntimeTargetParams) {
        return await callHost<SystemKillRuntimeResult>(
          'system.killRuntime',
          input,
        );
      },
    },
  };

  return true;
}
