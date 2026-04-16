import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import type {
  SageAppPackageManifest,
  SageAppUrlPreview,
  SageGrantedPermissions,
} from '@/bindings';
import {
  NetworkPermissionsSection,
} from '@/components/apps/permissions/NetworkPermissionsSection.tsx';
import {
  PersistentStoragePermissionSection
} from '@/components/apps/permissions/PersistentStoragePermissionSection.tsx';
import React from 'react';
import { WalletPermissionSection } from '@/components/apps/permissions/WalletPermissionSection.tsx';

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
  onGrantedPermissionsChange: React.Dispatch<
    React.SetStateAction<SageGrantedPermissions>
  >;
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

              {manifest.permissions ? (
                <>
                  {manifest.permissions.network && (
                    <NetworkPermissionsSection
                      wanted={manifest.permissions.network}
                      granted={grantedPermissions.network}
                      onGrantedPermissionsChange={onGrantedPermissionsChange}
                    />
                  )}

                  {manifest.permissions.persistent_storage && (
                    <PersistentStoragePermissionSection
                      wanted={manifest.permissions.persistent_storage}
                      granted={grantedPermissions.persistentStorage}
                      onGrantedPermissionsChange={onGrantedPermissionsChange}
                    />
                  )}

                  {manifest.permissions.wallet && (
                    <WalletPermissionSection
                      wanted={manifest.permissions.wallet}
                      granted={grantedPermissions.wallet}
                      onGrantedPermissionsChange={onGrantedPermissionsChange}
                    />
                  )}
                </>
              ) : (
                <div className='text-sm text-muted-foreground'>
                  This app does not request any permissions.
                </div>
              )}
            </div>

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
