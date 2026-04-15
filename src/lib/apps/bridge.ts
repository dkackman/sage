import { invoke } from '@tauri-apps/api/core';
import type {
  InstalledSageApp,
  SageBridgeFetchRequest,
  SageBridgeFetchResponse,
} from '@/bindings';

export interface SageBridgeRequest {
  channel: 'sage-bridge';
  id: string;
  method: string;
  params?: unknown;
}

export interface SageBridgeSuccessResponse {
  channel: 'sage-bridge';
  id: string;
  ok: true;
  result: unknown;
}

export interface SageBridgeErrorResponse {
  channel: 'sage-bridge';
  id: string;
  ok: false;
  error: {
    code: string;
    message: string;
  };
}

export type SageBridgeResponse =
  | SageBridgeSuccessResponse
  | SageBridgeErrorResponse;

export interface SageBridgeContext {
  app: InstalledSageApp;
}

export interface SageBridgeEventPayload {
  sourceLabel: string;
  appId: string;
  request: SageBridgeRequest;
}

function success(id: string, result: unknown): SageBridgeSuccessResponse {
  return {
    channel: 'sage-bridge',
    id,
    ok: true,
    result,
  };
}

function failure(
  id: string,
  code: string,
  message: string,
): SageBridgeErrorResponse {
  return {
    channel: 'sage-bridge',
    id,
    ok: false,
    error: {
      code,
      message,
    },
  };
}

export function isBridgeRequest(value: unknown): value is SageBridgeRequest {
  if (!value || typeof value !== 'object') {
    return false;
  }

  const maybe = value as Partial<SageBridgeRequest>;

  return (
    maybe.channel === 'sage-bridge' &&
    typeof maybe.id === 'string' &&
    typeof maybe.method === 'string'
  );
}

export function isBridgeResponse(value: unknown): value is SageBridgeResponse {
  if (!value || typeof value !== 'object') {
    return false;
  }

  const maybe = value as Partial<SageBridgeResponse>;
  return (
    maybe.channel === 'sage-bridge' &&
    typeof maybe.id === 'string' &&
    typeof maybe.ok === 'boolean'
  );
}

export async function handleBridgeRequest(
  ctx: SageBridgeContext,
  request: SageBridgeRequest,
): Promise<SageBridgeResponse> {
  try {
    switch (request.method) {
      case 'bridge.ping':
        return success(request.id, {
          ok: true,
          appId: ctx.app.id,
          appName: ctx.app.name,
        });

      case 'app.getInfo':
        return success(request.id, {
          id: ctx.app.id,
          name: ctx.app.name,
          version: ctx.app.version,
        });

      case 'sage.getPermissions':
        return success(request.id, ctx.app.permissions);

      case 'network.fetch': {
        if (!ctx.app.permissions.network) {
          return failure(
            request.id,
            'forbidden',
            'Network access denied by Sage',
          );
        }

        const params = request.params as SageBridgeFetchRequest;
        const response = await invoke<SageBridgeFetchResponse>(
          'bridge_fetch_http',
          { req: params },
        );

        return success(request.id, response);
      }

      default:
        return failure(
          request.id,
          'method_not_found',
          `Unknown bridge method: ${request.method}`,
        );
    }
  } catch (error) {
    const message =
      error instanceof Error ? error.message : 'Unknown Sage bridge error';

    return failure(request.id, 'internal_error', message);
  }
}

