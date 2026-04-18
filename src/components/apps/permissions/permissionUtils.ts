import type {
  SageAppPackageManifest,
  SageNetworkWhitelistEntry,
} from '@/bindings';

function sortNetworkEntries(
  entries: SageNetworkWhitelistEntry[],
): SageNetworkWhitelistEntry[] {
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
  return [...(manifest.permissions?.required ?? [])].sort((a, b) =>
    a.localeCompare(b),
  );
}

export function buildInitialGrantedNetworkWhitelist(
  manifest: SageAppPackageManifest,
): SageNetworkWhitelistEntry[] {
  return sortNetworkEntries(
    (manifest.network?.whitelist ?? []).filter((entry) => entry.required),
  );
}
