import type {
  SageAppPackageManifest,
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

export function buildFullyForbiddenPermissions(): string[] {
  return [];
}

export function buildInitialGrantedPermissions(
  manifest: SageAppPackageManifest,
): string[] {
  return [...(manifest.permissions?.capabilities?.required ?? [])].sort(
    (a, b) => a.localeCompare(b),
  );
}

export function buildInitialGrantedNetworkWhitelist(
  manifest: SageAppPackageManifest,
): SageNetworkPermissionTarget[] {
  return sortNetworkEntries(
    manifest.permissions?.network?.whitelist?.required ?? [],
  );
}
