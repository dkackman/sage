import { invoke } from '@tauri-apps/api/core';
import type {
  InstalledSageApp,
  SageStorageDatabaseDescription,
  SageStorageValueRecord,
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
  sourceLabel: string;
}

export interface SageBridgeEventPayload {
  sourceLabel: string;
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
          permissions: ctx.app.grantedPermissions,
        });

      case 'sage.getPermissions':
        return success(request.id, ctx.app.grantedPermissions);

      case 'storage.openDatabase': {
        const response = await invoke<{ name: string; version: number }>(
          'storage_open_database',
          {
            appId: ctx.app.id,
            req: request.params,
          },
        );

        return success(request.id, response);
      }

      case 'storage.deleteDatabase': {
        const dbName = request.params as string;

        await invoke('storage_delete_database', {
          appId: ctx.app.id,
          dbName,
        });

        return success(request.id, undefined);
      }

      case 'storage.describeDatabase': {
        const dbName = request.params as string;

        const response = await invoke<SageStorageDatabaseDescription>(
          'storage_describe_database',
          {
            appId: ctx.app.id,
            dbName,
          },
        );

        return success(request.id, response);
      }

      case 'storage.createObjectStore': {
        await invoke('storage_create_object_store', {
          appId: ctx.app.id,
          req: request.params,
        });

        return success(request.id, undefined);
      }

      case 'storage.createIndex': {
        await invoke('storage_create_index', {
          appId: ctx.app.id,
          req: request.params,
        });

        return success(request.id, undefined);
      }

      case 'storage.get': {
        const response = await invoke<string | null>('storage_get', {
          appId: ctx.app.id,
          req: request.params,
        });

        return success(request.id, response);
      }

      case 'storage.put': {
        await invoke('storage_put', {
          appId: ctx.app.id,
          req: request.params,
        });

        return success(request.id, undefined);
      }

      case 'storage.delete': {
        await invoke('storage_delete', {
          appId: ctx.app.id,
          req: request.params,
        });

        return success(request.id, undefined);
      }

      case 'storage.clear': {
        await invoke('storage_clear', {
          appId: ctx.app.id,
          req: request.params,
        });

        return success(request.id, undefined);
      }

      case 'storage.count': {
        const response = await invoke<number>('storage_count', {
          appId: ctx.app.id,
          req: request.params,
        });

        return success(request.id, response);
      }

      case 'storage.getAll': {
        const response = await invoke<SageStorageValueRecord[]>(
          'storage_get_all',
          {
            appId: ctx.app.id,
            req: request.params,
          },
        );

        return success(request.id, response);
      }

      case 'storage.getAllFromIndex': {
        const response = await invoke<SageStorageValueRecord[]>(
          'storage_get_all_from_index',
          {
            appId: ctx.app.id,
            req: request.params,
          },
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
