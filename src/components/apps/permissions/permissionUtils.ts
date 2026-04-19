import type {
  SageAppPackageManifest,
  SageGrantedPermissions,
  SageNetworkPermissionTarget,
} from '@/bindings';

function sortNetworkEntries(
  entries: SageNetworkPermissionTarget[],
): SageNetworkPermissionTarget[] {
  return [...entries].sort((a, b) => {
    const aKey = `${a.scheme}://${a.host}`;
    const bKey = `${b.scheme}://${b.host}`;
    return aKey.localeCompare(bKey);
  });
}

function sortStrings(values: string[]): string[] {
  return [...values].sort((a, b) => a.localeCompare(b));
}

export function buildEmptyGrantedPermissions(): SageGrantedPermissions {
  return {
    capabilities: [],
    network: {
      whitelist: [],
    },
  };
}

export function buildInitialGrantedPermissions(
  manifest: SageAppPackageManifest,
): SageGrantedPermissions {
  return {
    capabilities: sortStrings(
      manifest.permissions?.capabilities?.required ?? [],
    ),
    network: {
      whitelist: sortNetworkEntries(
        manifest.permissions?.network?.whitelist?.required ?? [],
      ),
    },
  };
}
