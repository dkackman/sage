import { invoke } from '@tauri-apps/api/core';
import type { TransactionResponse } from '@/bindings';
import type { BridgeMethodRegistry } from '../types';

export const walletBridgeMethods: BridgeMethodRegistry = {
  'wallet.sendXch': {
    approval: async ({ ctx, request }) => {
      if (
        ctx.app.grantedPermissions.capabilities.includes(
          'wallet.send_xch_auto_submit',

        )
      ) {
        return null;
      }

      return {
        kind: 'send_xch',
        app: ctx.app,
        sourceLabel: ctx.sourceLabel,
        requestId: request.id,
        params: request.params,
      };
    },

    async handle({ request }) {
      return await invoke<TransactionResponse>('send_xch', {
        req: {
          ...request.params,
          auto_submit: true,
        },
      });
    },
  },
} satisfies BridgeMethodRegistry;
