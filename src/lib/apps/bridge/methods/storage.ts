import { invoke } from '@tauri-apps/api/core';
import type {
  SageStorageDatabaseDescription,
  SageStorageValueRecord,
} from '@/bindings';
import { BridgePermissionError } from '../permissions';
import type { BridgeMethodRegistry } from '../types';

async function ensureStoragePermission({
  ctx,
}: {
  ctx: { app: { grantedPermissions: string[] } };
}) {
  if (!ctx.app.grantedPermissions.includes('persistent_storage')) {
    throw new BridgePermissionError(
      'App does not have persistent_storage permission',
    );
  }
}

const storagePermission = {
  kind: 'custom' as const,
  check: ensureStoragePermission,
};

export const storageBridgeMethods = {
  'storage.openDatabase': {
    permission: storagePermission,
    async handle({ ctx, request }) {
      return await invoke<{ name: string; version: number }>(
        'storage_open_database',
        {
          appId: ctx.app.id,
          req: request.params,
        },
      );
    },
  },

  'storage.deleteDatabase': {
    permission: storagePermission,
    async handle({ ctx, request }) {
      return await invoke('storage_delete_database', {
        appId: ctx.app.id,
        dbName: request.params,
      });
    },
  },

  'storage.describeDatabase': {
    permission: storagePermission,
    async handle({ ctx, request }) {
      return await invoke<SageStorageDatabaseDescription>(
        'storage_describe_database',
        {
          appId: ctx.app.id,
          dbName: request.params,
        },
      );
    },
  },

  'storage.createObjectStore': {
    permission: storagePermission,
    async handle({ ctx, request }) {
      await invoke('storage_create_object_store', {
        appId: ctx.app.id,
        req: request.params,
      });
      return undefined;
    },
  },

  'storage.createIndex': {
    permission: storagePermission,
    async handle({ ctx, request }) {
      await invoke('storage_create_index', {
        appId: ctx.app.id,
        req: request.params,
      });
      return undefined;
    },
  },

  'storage.get': {
    permission: storagePermission,
    async handle({ ctx, request }) {
      return await invoke<string | null>('storage_get', {
        appId: ctx.app.id,
        req: request.params,
      });
    },
  },

  'storage.put': {
    permission: storagePermission,
    async handle({ ctx, request }) {
      await invoke('storage_put', {
        appId: ctx.app.id,
        req: request.params,
      });
      return undefined;
    },
  },

  'storage.delete': {
    permission: storagePermission,
    async handle({ ctx, request }) {
      await invoke('storage_delete', {
        appId: ctx.app.id,
        req: request.params,
      });
      return undefined;
    },
  },

  'storage.clear': {
    permission: storagePermission,
    async handle({ ctx, request }) {
      await invoke('storage_clear', {
        appId: ctx.app.id,
        req: request.params,
      });
      return undefined;
    },
  },

  'storage.count': {
    permission: storagePermission,
    async handle({ ctx, request }) {
      return await invoke<number>('storage_count', {
        appId: ctx.app.id,
        req: request.params,
      });
    },
  },

  'storage.getAll': {
    permission: storagePermission,
    async handle({ ctx, request }) {
      return await invoke<SageStorageValueRecord[]>('storage_get_all', {
        appId: ctx.app.id,
        req: request.params,
      });
    },
  },

  'storage.getAllFromIndex': {
    permission: storagePermission,
    async handle({ ctx, request }) {
      return await invoke<SageStorageValueRecord[]>(
        'storage_get_all_from_index',
        {
          appId: ctx.app.id,
          req: request.params,
        },
      );
    },
  },
} satisfies BridgeMethodRegistry;
