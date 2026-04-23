import type { SageSystemClient } from './types';

export function isSageSystemRuntimeAvailable(): boolean {
  const w = window as Window &
    typeof globalThis & {
      __TAURI__?: unknown;
    };

  return !!w.__TAURI__;
}

function getClientFromWindow(): SageSystemClient | undefined {
  if (typeof window === 'undefined') {
    return undefined;
  }

  return (
    window as Window &
      typeof globalThis & {
        __SAGE_SYSTEM__?: SageSystemClient;
      }
  ).__SAGE_SYSTEM__;
}

export function isSageSystemBridgeInitialized(): boolean {
  return !!getClientFromWindow();
}

export function hasSageSystemBridge(): boolean {
  return !!getClientFromWindow();
}

function getClientOrThrow(): SageSystemClient {
  const client = getClientFromWindow();

  if (!client) {
    throw new Error(
      'Sage system bridge is unavailable. Did you call initSageSystemRuntimeBridge() in app startup?',
    );
  }

  return client;
}

export async function createSageSystemClient(): Promise<SageSystemClient> {
  return getClientOrThrow();
}

export function getSageSystemClientSync(): SageSystemClient {
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
