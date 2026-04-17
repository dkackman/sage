import type {
    SageAppInfo,
    SageBridgeEventPayload,
    SageBridgeRequest,
    SageBridgeResponse,
    SageBridgeSendPayload,
    SageBridgeVersion,
    SageClient,
    SageWalletSendXchRequest,
    TransactionResponse,
} from './types';

export const SAGE_BRIDGE_VERSION: SageBridgeVersion = 'v1';

type SageListenEvent<T = unknown> = {
    payload: T;
};

type SageUnlisten = () => void;

type SageWebviewHandle = {
    label: string;
    emitTo(target: string, event: string, payload: unknown): Promise<void>;
    listen<T = unknown>(
        event: string,
        handler: (event: SageListenEvent<T>) => void,
    ): Promise<SageUnlisten>;
};

type SageTauriGlobal = {
    webview?: {
        getCurrentWebview(): SageWebviewHandle;
    };
};

type SageWindow = Window &
    typeof globalThis & {
    __SAGE__?: SageClient;
    __SAGE_APP_INFO__?: SageAppInfo;
    __SAGE_RUNTIME_BRIDGE_INITIALIZED__?: boolean;
    __TAURI__?: SageTauriGlobal;
};

function getSageWindow(): SageWindow {
    return window as SageWindow;
}

function buildFallbackAppInfo(): SageAppInfo {
    const appId =
        new URL(window.location.href).searchParams.get('__sage_app_id') ??
        'unknown-app';

    return {
        id: appId,
        name: document.title || appId,
        version: '0.0.0',
        requestedPermissions: {
            required: [],
            optional: [],
        },
        grantedPermissions: [],
        network: [],
    };
}

function tryGetCurrentWebview(): SageWebviewHandle | null {
    const tauri = getSageWindow().__TAURI__;
    if (!tauri?.webview?.getCurrentWebview) {
        return null;
    }

    return tauri.webview.getCurrentWebview();
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

    const sourceLabel = webview.label;
    const bridgeListeners = new Set<(event: unknown) => void>();

    webview
        .listen('sage-bridge:event', (event: SageListenEvent) => {
            const data = event.payload;
            if (!data || (data as { channel?: string }).channel !== 'sage-bridge') {
                return;
            }

            for (const listener of bridgeListeners) {
                try {
                    listener(data);
                } catch (error: unknown) {
                    console.error('Sage bridge event listener failed:', error);
                }
            }
        })
        .catch((error: unknown) => {
            console.error('Failed to subscribe to sage-bridge:event:', error);
        });

    webview
        .listen('sage-lifecycle:before-stop', (event: SageListenEvent) => {
            try {
                window.dispatchEvent(
                    new CustomEvent('sage:lifecycle:before-stop', {
                        detail: event.payload,
                    }),
                );
            } catch (error: unknown) {
                console.error('Failed to dispatch before-stop lifecycle event', error);
            }
        })
        .catch((error: unknown) => {
            console.error('Failed to subscribe to sage-lifecycle:before-stop:', error);
        });

    async function callHost<T>(method: string, params?: unknown): Promise<T> {
        const id = `sage-${Date.now()}-${Math.random().toString(36).slice(2)}`;

        return await new Promise<T>((resolve, reject) => {
            void (async () => {
                let settled = false;

                const timeoutId = window.setTimeout(() => {
                    if (settled) {
                        return;
                    }

                    settled = true;
                    unlistenPromise
                        .then((unlisten: SageUnlisten) => unlisten())
                        .catch(() => {});
                    reject(new Error(`Sage bridge timeout for ${method}`));
                }, 15000);

                const unlistenPromise = webview.listen(
                    'sage-bridge:response',
                    (event: SageListenEvent<SageBridgeResponse>) => {
                        const data = event.payload;

                        if (
                            !data ||
                            data.channel !== 'sage-bridge' ||
                            data.bridgeVersion !== SAGE_BRIDGE_VERSION ||
                            data.id !== id
                        ) {
                            return;
                        }

                        if (settled) {
                            return;
                        }

                        settled = true;
                        window.clearTimeout(timeoutId);
                        unlistenPromise
                            .then((unlisten: SageUnlisten) => unlisten())
                            .catch(() => {});

                        if (data.ok) {
                            resolve(data.result as T);
                        } else {
                            reject(new Error(data.error?.message || 'Unknown Sage bridge error'));
                        }
                    },
                );

                try {
                    const request: SageBridgeRequest = {
                        channel: 'sage-bridge',
                        bridgeVersion: SAGE_BRIDGE_VERSION,
                        id,
                        method,
                        params,
                    };

                    const payload: SageBridgeEventPayload = {
                        sourceLabel,
                        request,
                    };

                    await webview.emitTo('main', 'sage-bridge:request', payload);
                } catch (error: unknown) {
                    if (settled) {
                        return;
                    }

                    settled = true;
                    window.clearTimeout(timeoutId);
                    unlistenPromise
                        .then((unlisten: SageUnlisten) => unlisten())
                        .catch(() => {});
                    reject(error instanceof Error ? error : new Error(String(error)));
                }
            })();
        });
    }

    w.__SAGE__ = {
        initialAppInfo: w.__SAGE_APP_INFO__ ?? buildFallbackAppInfo(),
        app: {
            async bridgePing() {
                return await callHost<unknown>('bridge.ping');
            },

            async bridgeSend(input: SageBridgeSendPayload) {
                return await callHost<unknown>('bridge.send', input);
            },

            async getInfo() {
                return await callHost<SageAppInfo>('app.getInfo');
            },

            async getPermissions() {
                return await callHost<string[]>('sage.getPermissions');
            },
        },

        lifecycle: {
            onBeforeStop(handler) {
                const listener = (event: Event) => {
                    const custom = event as CustomEvent;
                    handler((custom.detail ?? {}) as {
                        reason?: string;
                        appId?: string;
                        runtimeId?: string;
                    });
                };

                window.addEventListener(
                    'sage:lifecycle:before-stop',
                    listener as EventListener,
                );

                return () => {
                    window.removeEventListener(
                        'sage:lifecycle:before-stop',
                        listener as EventListener,
                    );
                };
            },
        },

        wallet: {
            async sendXch(input: SageWalletSendXchRequest) {
                return await callHost<TransactionResponse>('wallet.sendXch', input);
            },
        },
    };
    return true;
}
