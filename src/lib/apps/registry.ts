import { SageAppManifest } from './types';

export const BUILTIN_APPS: SageAppManifest[] = [
  {
    id: 'chia.vanity',
    name: 'Vanity Address Generator',
    version: '0.1.0',
    description:
      'Brute-force Chia receive address prefixes, save results, and later sweep coins from matching derivation indexes.',
    permissions: [
      'wallet.read.addresses',
      'wallet.read.balance',
      'wallet.tx.create',
      'wallet.tx.submit',
      'storage.readwrite',
    ],
    verified: true,
    builtIn: true,
  },
];

export function getBuiltinApp(appId: string): SageAppManifest | undefined {
  return BUILTIN_APPS.find((app) => app.id === appId);
}

