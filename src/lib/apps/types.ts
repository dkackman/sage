export interface SageAppPermissionMap {
  'wallet.read.addresses': true;
  'wallet.read.balance': true;
  'wallet.tx.create': true;
  'wallet.tx.submit': true;
  'storage.readwrite': true;
}

export type SageAppPermission = keyof SageAppPermissionMap;

export interface SageAppManifest {
  id: string;
  name: string;
  version: string;
  description: string;
  entry: string;
  permissions: SageAppPermission[];
  verified?: boolean;
  publisher?: string | null;
  source?: 'local' | 'marketplace' | 'manual';
  installDir?: string | null;
  icon?: string | null;
}

export interface InstalledAppsState {
  [appId: string]: SageAppManifest;
}

