import type { BridgeMethodRegistry } from './types';
import { systemBridgeMethods } from './methods/system';
import { walletBridgeMethods } from './methods/wallet';

export const bridgeMethods: BridgeMethodRegistry = {
  ...systemBridgeMethods,
  ...walletBridgeMethods,
} satisfies BridgeMethodRegistry;
