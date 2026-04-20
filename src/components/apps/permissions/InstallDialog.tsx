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
      <DialogContent className='max-w-md'>
        <DialogHeader>
          <DialogTitle>Install app</DialogTitle>
        </DialogHeader>

        <div className='space-y-5'>
          <div className='space-y-1 text-sm text-muted-foreground'>
            <div>{manifest.name}</div>
            <div>v{manifest.version}</div>
          </div>

          <PermissionsEditor
            app={previewApp}
            grantedPermissions={grantedPermissions}
            onGrantedPermissionsChange={onGrantedPermissionsChange}
          />

          {error ? (
            <div className='text-sm text-destructive'>{error}</div>
          ) : null}
        </div>

        <DialogFooter>
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
