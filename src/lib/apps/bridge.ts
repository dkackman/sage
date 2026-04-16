import { invoke } from '@tauri-apps/api/core';
import type {
  InstalledSageApp,
  SageStorageClearRequest,
  SageStorageCountRequest,
  SageStorageCreateIndexRequest,
  SageStorageCreateObjectStoreRequest,
  SageStorageDatabaseDescription,
  SageStorageDeleteRequest,
  SageStorageGetAllFromIndexRequest,
  SageStorageGetAllRequest,
  SageStorageGetRequest,
  SageStorageOpenDatabaseRequest,
  SageStoragePutRequest,
  SageStorageValueRecord,
  SendXch,
} from '@/bindings';

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

interface SageBridgeRequestParamsMap {
  'bridge.ping': undefined;
  'app.getInfo': undefined;
  'sage.getPermissions': undefined;
  'storage.openDatabase': SageStorageOpenDatabaseRequest;
  'storage.deleteDatabase': string;
  'storage.describeDatabase': string;
  'storage.createObjectStore': SageStorageCreateObjectStoreRequest;
  'storage.createIndex': SageStorageCreateIndexRequest;
  'storage.get': SageStorageGetRequest;
  'storage.put': SageStoragePutRequest;
  'storage.delete': SageStorageDeleteRequest;
  'storage.clear': SageStorageClearRequest;
  'storage.count': SageStorageCountRequest;
  'storage.getAll': SageStorageGetAllRequest;
  'storage.getAllFromIndex': SageStorageGetAllFromIndexRequest;
  'wallet.sendXch': SendXch;
}

type SageBridgeMethod = keyof SageBridgeRequestParamsMap;

type SageBridgeRequestForMethod<M extends SageBridgeMethod> =
  SageBridgeRequestParamsMap[M] extends undefined
    ? {
        channel: 'sage-bridge';
        id: string;
        method: M;
      }
    : {
        channel: 'sage-bridge';
        id: string;
        method: M;
        params: SageBridgeRequestParamsMap[M];
      };

export type SageBridgeRequest = {
  [M in SageBridgeMethod]: SageBridgeRequestForMethod<M>;
}[SageBridgeMethod];

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
        const response = await invoke('storage_delete_database', {
          appId: ctx.app.id,
          dbName: request.params,
        });

        return success(request.id, response);
      }

      case 'storage.describeDatabase': {
        const response = await invoke<SageStorageDatabaseDescription>(
          'storage_describe_database',
          {
            appId: ctx.app.id,
            dbName: request.params,
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

      case 'wallet.sendXch': {
        const effectiveParams: SendXch = {
          ...request.params,
          auto_submit: ctx.app.grantedPermissions.wallet?.sendXchAutoSubmit
            ? request.params.auto_submit
            : false,
        };

        const response = await invoke<unknown>('send_xch', {
          req: effectiveParams,
        });

        return success(request.id, response);
      }

      default: {
        throw new Error(`Unhandled bridge request: ${String(request)}`);
      }
    }
  } catch (error) {
    const message =
      error instanceof Error ? error.message : 'Unknown Sage bridge error';

    return failure(request.id, 'internal_error', message);
  }
}
