import type {
  SageAppPackageManifest,
  SageGrantedNetworkPermissionEntry,
  SageGrantedPermissions,
  SageNetworkPermissionEntry,
} from '@/bindings';

export function buildFullyForbiddenPermissions(): SageGrantedPermissions {
  return {
    network: [],
    persistentStorage: false,
    wallet: {
      sendXch: false,
      sendXchAutoSubmit: false,
    },
  };
}

export function buildInitialGrantedPermissions(
  manifest: SageAppPackageManifest,
): SageGrantedPermissions {
  return {
    network: (manifest.permissions?.network ?? []).map((entry) => ({
      scheme: entry.scheme,
      host: entry.host,
    })),
    persistentStorage: !!manifest.permissions?.persistent_storage,
    wallet: {
      sendXch: !!manifest.permissions?.wallet?.sendXch,
      sendXchAutoSubmit: !!manifest.permissions?.wallet?.sendXchAutoSubmit,
    },
  };
}

export function isNetworkGranted(
  entry: SageNetworkPermissionEntry,
  granted: SageGrantedNetworkPermissionEntry[],
): boolean {
  return granted.some(
    (g) => g.scheme === entry.scheme && g.host === entry.host,
  );
}

