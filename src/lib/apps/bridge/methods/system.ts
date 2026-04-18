import type { BridgeMethodRegistry } from '../types';
import { acceptSandboxBridgeSend } from '@/lib/apps/sandboxRuntimeStore';

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
      acceptSandboxBridgeSend({
        appId: ctx.app.id,
        payload: request.params,
      });

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
        requestedPermissions: ctx.app.requestedPermissions,
        grantedPermissions: ctx.app.grantedPermissions,
        network:
          ctx.app.activeSnapshot.manifest.permissions?.network?.whitelist
            ?.required ?? [],
      };
    },
  },

  'sage.getPermissions': {
    permission: { kind: 'none' },
    async handle({ ctx }) {
      return ctx.app.grantedPermissions.capabilities;
    },
  },
} satisfies BridgeMethodRegistry;
