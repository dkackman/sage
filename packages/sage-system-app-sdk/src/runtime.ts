import type {
  RuntimeTargetParams,
  SageAppRuntimeRecord,
  SageSystemBridgeResponse,
  SageSystemClient,
  SageSystemBridgeVersion,
  SystemKillRuntimeResult,
} from './types';
import { createBridgeRuntimeCore } from '@sage-app/sdk';

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

function getSageWindow(): SageSystemWindow {
  return window as SageSystemWindow;
}

export function initSageSystemRuntimeBridge(): boolean {
  const w = getSageWindow();

  if (w.__SAGE_SYSTEM__) {
    return true;
  }

  if (w.__SAGE_SYSTEM_RUNTIME_BRIDGE_INITIALIZED__) {
    return true;
  }

  const core = createBridgeRuntimeCore({
    channel: SAGE_SYSTEM_BRIDGE_CHANNEL,
    version: SAGE_SYSTEM_BRIDGE_VERSION,
    invokeCommand: 'apps_invoke_system_bridge',
    requestIdPrefix: 'sage-system',
  });

  if (!core) {
    return false;
  }

  const webview = core.webview as SageWebviewHandle;
  const callHost = core.callHost;
  const rejectAllPending = core.rejectAllPending;

  w.__SAGE_SYSTEM_RUNTIME_BRIDGE_INITIALIZED__ = true;

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

        const pending = core.pendingRequests.get(data.id);
        if (!pending) {
          return;
        }

        core.pendingRequests.delete(data.id);
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

  w.__SAGE_SYSTEM__ = {
    runtimeManager: {
      async listRuntimes() {
        return await callHost<SageAppRuntimeRecord[]>(
          'runtimeManager.listRuntimes',
        );
      },

      async focusRuntime(input: RuntimeTargetParams) {
        return await callHost<SageAppRuntimeRecord>(
          'runtimeManager.focusRuntime',
          input,
        );
      },

      async hideRuntime(input: RuntimeTargetParams) {
        return await callHost<SageAppRuntimeRecord>(
          'runtimeManager.hideRuntime',
          input,
        );
      },

      async killRuntime(input: RuntimeTargetParams) {
        return await callHost<SystemKillRuntimeResult>(
          'runtimeManager.killRuntime',
          input,
        );
      },
    },
  };

  return true;
}
