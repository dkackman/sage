import type { BridgeMethodRegistry } from '../types';

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
