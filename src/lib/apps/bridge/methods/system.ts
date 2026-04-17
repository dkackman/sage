import type { BridgeMethodRegistry } from '../types';
import { invoke } from '@tauri-apps/api/core';

export const systemBridgeMethods = {
  'bridge.ping': {
    permission: { kind: 'none' },
    async handle({ ctx }) {
      return {
        ok: true,
        appId: ctx.app.id,
        appName: ctx.app.name,
      };
    },
  },

  'bridge.send': {
    permission: { kind: 'none' },
    async handle({ ctx, request }) {
      if (request.params.kind === "sandbox_report") {
        await invoke('sandbox_bridge_send', {
          appId: ctx.app.id,
          payload: request.params,
        });
      }

      return { ok: true };
    },
  },

  'app.getInfo': {
    permission: { kind: 'none' },
    async handle({ ctx }) {
      return {
        id: ctx.app.id,
        name: ctx.app.name,
        version: ctx.app.version,
        permissions: ctx.app.grantedPermissions,
      };
    },
  },

  'sage.getPermissions': {
    permission: { kind: 'none' },
    async handle({ ctx }) {
      return ctx.app.grantedPermissions;
    },
  },
} satisfies BridgeMethodRegistry;
