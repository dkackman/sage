import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import type { SageAppPackageManifest, SageAppUrlPreview } from '@/bindings';
import React from 'react';
import { AppPermissions } from '@/components/apps/permissions/AppPermissions';

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
  grantedPermissions: string[];
  onGrantedPermissionsChange: React.Dispatch<React.SetStateAction<string[]>>;
  onCancel: () => void;
  onConfirm: () => void;
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
  const manifest =
    source?.kind === 'zip'
      ? source.manifest
      : (source?.preview.manifest ?? null);

  const networkEntries = manifest?.network?.whitelist ?? [];

  return (
    <Dialog open={!!manifest} onOpenChange={(open) => !open && onCancel()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Install {manifest?.name}</DialogTitle>
        </DialogHeader>

        {manifest ? (
          <div className='space-y-5'>
            <div className='space-y-1 text-sm text-muted-foreground'>
              <div>v{manifest.version}</div>

              {source?.kind === 'url' ? (
                <>
                  <div className='break-all'>URL: {source.preview.appUrl}</div>
                  <div className='break-all'>
                    Manifest: {source.preview.manifestUrl}
                  </div>
                </>
              ) : null}
            </div>

            <div className='space-y-3'>
              <h3 className='text-sm font-medium'>Permissions</h3>

              <AppPermissions
                permissions={manifest.permissions}
                grantedPermissions={grantedPermissions}
                editable
                onGrantedPermissionsChange={onGrantedPermissionsChange}
              />
            </div>

            {networkEntries.length > 0 ? (
              <div className='space-y-2'>
                <div className='text-sm font-medium'>Network allowlist</div>

                <div className='space-y-2 rounded-md border p-3'>
                  {networkEntries.map((entry) => (
                    <div
                      key={`${entry.scheme}://${entry.host}`}
                      className='text-xs font-mono'
                    >
                      {entry.scheme}://{entry.host}
                      {entry.required ? ' (required)' : ''}
                    </div>
                  ))}
                </div>
              </div>
            ) : null}

            {error ? (
              <div className='text-sm text-destructive'>{error}</div>
            ) : null}
          </div>
        ) : null}

        <DialogFooter>
          <Button variant='outline' onClick={onCancel}>
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
