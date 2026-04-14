export interface SageAppPermissions {
  network: boolean;
  persistent_storage: boolean;
}

export interface InstalledSageApp {
  id: string;
  name: string;
  version: string;
  installDir: string;
  entryFile: string;
  iconFile: string;
  permissions: SageAppPermissions;
}

