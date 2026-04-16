import type { BridgeMethodRegistry } from './types';
import { systemBridgeMethods } from './methods/system';
import { storageBridgeMethods } from './methods/storage';
import { walletBridgeMethods } from './methods/wallet';

export const bridgeMethods = {
  ...systemBridgeMethods,
  ...storageBridgeMethods,
  ...walletBridgeMethods,
} satisfies BridgeMethodRegistry;
