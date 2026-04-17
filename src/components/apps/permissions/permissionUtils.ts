import type { SageAppPackageManifest } from '@/bindings';

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
