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
  SageBridgeResponse,
  SageBridgeRuntimeEvent,
  SageBridgeSendPayload,
  SageBridgeVersion,
  SageClient,
  SageLifecycleBeforeStopDetail,
  SetBeforeStopListenerParams,
  TransactionResponse,
  WalletSendXchParams,
  GetKey,
  GetKeyResponse,
  GetKeysResponse,
  GetSecretKey,
  GetSecretKeyResponse,
} from './types';
import { createBridgeRuntimeCore } from './bridge-runtime-core';

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

function isObject(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object';
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

  const core = createBridgeRuntimeCore({
    channel: 'sage-bridge',
    version: SAGE_BRIDGE_VERSION,
    invokeCommand: 'apps_invoke_bridge',
    requestIdPrefix: 'sage',
  });

  if (!core) {
    return false;
  }

  const webview = core.webview as SageWebviewHandle;
  const callHost = core.callHost;
  const rejectAllPending = core.rejectAllPending;

  w.__SAGE_RUNTIME_BRIDGE_INITIALIZED__ = true;

  const beforeStopHandlers = new Set<
    (detail: BeforeStopPublicDetail) => void | Promise<void>
  >();
  let beforeStopRegistered = false;

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
              'app:granted-capabilities-change',
              { detail: data },
            ),
          );
          return;
        }

        if (isGrantedNetworkWhitelistChangeEvent(data)) {
          window.dispatchEvent(
            new CustomEvent<GrantedNetworkWhitelistChangeEvent>(
              'app:granted-network-whitelist-change',
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
    .listen<SageBridgeResponse>(
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
        return await callHost<string[]>('app.getCapabilities');
      },

      async requestCapabilityGrant(input: RequestCapabilityGrantParams) {
        return await callHost<RequestCapabilityGrantResult>(
          'app.requestCapabilityGrant',
          input,
        );
      },

      async requestNetworkWhitelistGrant(
        input: RequestNetworkWhitelistGrantParams,
      ) {
        return await callHost<RequestNetworkWhitelistGrantResult>(
          'app.requestNetworkWhitelistGrant',
          input,
        );
      },

      onGrantedCapabilitiesChange(handler) {
        const listener = (event: Event) => {
          const custom = event as CustomEvent<GrantedCapabilitiesChangeEvent>;
          handler(custom.detail);
        };

        window.addEventListener(
          'app:granted-capabilities-change',
          listener as EventListener,
        );

        return () => {
          window.removeEventListener(
            'app:granted-capabilities-change',
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
          'app:granted-network-whitelist-change',
          listener as EventListener,
        );

        return () => {
          window.removeEventListener(
            'app:granted-network-whitelist-change',
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
      async getKeys() {
        return await callHost<GetKeysResponse>('wallet.getKeys');
      },
      async getKey(input?: GetKey) {
        return await callHost<GetKeyResponse>('wallet.getKey', input);
      },
      async getSecretKey(input: GetSecretKey) {
        return await callHost<GetSecretKeyResponse>(
          'wallet.getSecretKey',
          input,
        );
      },
      async sendXch(input: WalletSendXchParams) {
        return await callHost<TransactionResponse>('wallet.sendXch', input);
      },
    },
  };

  return true;
}
