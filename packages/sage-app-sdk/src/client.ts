import type { SageClient } from './types';

export function isSageRuntimeAvailable(): boolean {
    const w = window as Window & typeof globalThis & {
        __TAURI__?: unknown;
    };

    return !!w.__TAURI__;
}

function getClientFromWindow(): SageClient | undefined {
    if (typeof window === 'undefined') {
        return undefined;
    }

    return window.__SAGE__;
}

export function isSageBridgeInitialized(): boolean {
    return !!getClientFromWindow();
}

export function hasSageBridge(): boolean {
    return !!getClientFromWindow();
}

function getClientOrThrow(): SageClient {
    const client = getClientFromWindow();

    if (!client) {
        throw new Error(
            'Sage bridge is unavailable. Did you call initSageRuntimeBridge() in app startup?',
        );
    }

    return client;
}

export async function createSageClient(): Promise<SageClient> {
    return getClientOrThrow();
}

export function getSageClientSync(): SageClient {
    return getClientOrThrow();
}

function isObject(value: unknown): value is Record<string, unknown> {
    return !!value && typeof value === 'object';
}

export function formatSageError(err: unknown): string {
    if (err instanceof Error) {
        return err.message;
    }

    if (typeof err === 'string') {
        return err;
    }

    if (isObject(err)) {
        if (typeof err.message === 'string') {
            return err.message;
        }

        if (typeof err.reason === 'string') {
            return err.reason;
        }

        try {
            return JSON.stringify(err, null, 2);
        } catch {
            return 'Unknown Sage error';
        }
    }

    return String(err);
}
