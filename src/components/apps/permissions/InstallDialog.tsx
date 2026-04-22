import type {
  InstalledSageApp,
  SageAppPackageManifest,
  SageAppUrlPreview,
  SageGrantedPermissions,
} from '@/bindings';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { PermissionsEditor } from '@/components/apps/permissions/PermissionsEditor';
import { Globe, Package } from 'lucide-react';

type InstallSource =
  | {
      kind: 'zip';
      zipPath: string;
      manifest: SageAppPackageManifest;
    }
  | {
      kind: 'url';
      appUrl: string;
      preview: SageAppUrlPreview;
    };

interface Props {
  source: InstallSource | null;
  error: string | null;
  installing: boolean;
  grantedPermissions: SageGrantedPermissions;
  onGrantedPermissionsChange: (next: SageGrantedPermissions) => void;
  onCancel: () => void;
  onConfirm: () => void;
}

function buildPreviewApp(
  manifest: SageAppPackageManifest,
  grantedPermissions: SageGrantedPermissions,
): InstalledSageApp {
  return {
    id: '__install_preview__',
    originId: '__install_preview__',
    name: manifest.name,
    version: manifest.version,
    installDir: '',
    entryFile: manifest.entry ?? 'index.html',
    iconFile: manifest.icon ?? 'icon.png',
    requestedPermissions: manifest.permissions ?? {
      network: {
        whitelist: {
          required: [],
          optional: [],
        },
      },
      capabilities: {
        required: [],
        optional: [],
      },
    },
    grantedPermissions,
    capabilityFlags: {
      hasSecretAccess: false,
      hasExternalAccess: false,
      storageMayContainSecrets: false,
      isolated: false,
    },
    storage: { kind: 'unmanaged' },
    source: { kind: 'zip' },
    activeSnapshot: {
      manifestHash: '__install_preview__',
      snapshotDir: '',
      totalBytes: 0,
      manifest,
    },
    pendingUpdate: null,
  };
}

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) {
    return 'Unknown';
  }

  if (bytes < 1024) {
    return `${bytes} B`;
  }

  const units = ['KB', 'MB', 'GB', 'TB'];
  let value = bytes / 1024;
  let unitIndex = 0;

  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }

  const digits = value >= 100 ? 0 : value >= 10 ? 1 : 2;
  return `${value.toFixed(digits)} ${units[unitIndex]}`;
}

function resolveInstallIconUrl(
  source: InstallSource,
  manifest: SageAppPackageManifest,
): string | null {
  // 1. candidate paths (ordered)
  const candidates: string[] = [];

  if (manifest.icon) {
    candidates.push(manifest.icon);
  }

  candidates.push('icon.png');

  const existing = candidates.find((candidate) =>
    manifest.files.some((f) => f.path === candidate),
  );

  if (!existing) {
    return null;
  }

  if (source.kind === 'url') {
    try {
      return new URL(existing, source.preview.manifestUrl).toString();
    } catch {
      return null;
    }
  }

  return null;
}

function computeManifestSize(manifest: SageAppPackageManifest): number {
  return manifest.files.reduce((sum, f) => sum + (f.size ?? 0), 0);
}

function AppIcon({ name, iconUrl }: { name: string; iconUrl: string | null }) {
  const initial = name.trim().charAt(0).toUpperCase() || 'A';

  if (iconUrl) {
    return (
      <div className='h-16 w-16 overflow-hidden rounded-2xl border bg-background shadow-sm'>
        <img src={iconUrl} alt='' className='h-full w-full object-cover' />
      </div>
    );
  }

  return (
    <div className='flex h-16 w-16 items-center justify-center rounded-2xl border bg-muted/30 text-lg font-semibold shadow-sm'>
      {initial}
    </div>
  );
}

function InstallAppSummary({
  source,
  manifest,
}: {
  source: InstallSource;
  manifest: SageAppPackageManifest;
}) {
  const iconUrl = resolveInstallIconUrl(source, manifest);
  const previewSizeBytes = computeManifestSize(manifest);

  return (
    <div className='rounded-2xl border bg-muted/20 p-4'>
      <div className='flex items-start gap-4'>
        <AppIcon name={manifest.name} iconUrl={iconUrl} />

        <div className='min-w-0 flex-1'>
          <div className='flex flex-wrap items-center gap-2'>
            <div className='truncate text-xl font-semibold'>
              {manifest.name}
            </div>

            <span className='rounded-full border px-2 py-0.5 text-xs text-muted-foreground'>
              v{manifest.version}
            </span>

            <span className='rounded-full border px-2 py-0.5 text-xs text-muted-foreground'>
              {source.kind === 'url' ? 'URL install' : 'ZIP install'}
            </span>
          </div>

          <div className='mt-3 grid gap-2 text-sm text-muted-foreground sm:grid-cols-2'>
            <div className='flex items-center gap-2'>
              {source.kind === 'url' ? (
                <Globe className='h-4 w-4' />
              ) : (
                <Package className='h-4 w-4' />
              )}
              <span className='truncate'>
                {source.kind === 'url' ? source.appUrl : source.zipPath}
              </span>
            </div>

            <div>
              <span className='text-foreground'>Size:</span>{' '}
              {previewSizeBytes !== null
                ? formatBytes(previewSizeBytes)
                : 'Unknown'}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

export function InstallPermissionsDialog({
  source,
  error,
  installing,
  grantedPermissions,
  onGrantedPermissionsChange,
  onCancel,
  onConfirm,
}: Props) {
  const open = !!source;

  if (!source) {
    return (
      <Dialog open={open} onOpenChange={(nextOpen) => !nextOpen && onCancel()}>
        <DialogContent />
      </Dialog>
    );
  }

  const manifest =
    source.kind === 'zip' ? source.manifest : source.preview.manifest;

  const previewApp = buildPreviewApp(manifest, grantedPermissions);

  return (
    <Dialog open={open} onOpenChange={(nextOpen) => !nextOpen && onCancel()}>
      <DialogContent className='max-w-lg'>
        <DialogHeader className='pb-1'>
          <DialogTitle>Install app</DialogTitle>
        </DialogHeader>

        <div className='space-y-5'>
          <InstallAppSummary source={source} manifest={manifest} />

          <PermissionsEditor
            app={previewApp}
            grantedPermissions={grantedPermissions}
            onGrantedPermissionsChange={onGrantedPermissionsChange}
          />

          {error ? (
            <div className='rounded-lg border border-destructive/30 bg-destructive/5 px-3 py-2 text-sm text-destructive'>
              {error}
            </div>
          ) : null}
        </div>

        <DialogFooter className='gap-2'>
          <Button variant='outline' onClick={onCancel} disabled={installing}>
            Cancel
          </Button>

          <Button onClick={onConfirm} disabled={installing}>
            {installing ? 'Installing...' : 'Install'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

