import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import type { InstalledSageApp } from '@/bindings';
import React from 'react';
import { AppPermissions } from '@/components/apps/permissions/AppPermissions';

interface Props {
  app: InstalledSageApp | null;
  open: boolean;
  title: string;
  description?: string | null;
  error: string | null;
  submitting: boolean;
  grantedPermissions: string[];
  onGrantedPermissionsChange: React.Dispatch<React.SetStateAction<string[]>>;
  onCancel: () => void;
  onConfirm: () => void;
}

export function PermissionsDialog({
  app,
  open,
  title,
  description,
  error,
  submitting,
  grantedPermissions,
  onGrantedPermissionsChange,
  onCancel,
  onConfirm,
}: Props) {
  const manifest =
    app?.pendingUpdate?.manifest ?? app?.activeSnapshot.manifest ?? null;
  const requestedPermissions =
    app?.pendingUpdate?.manifest.permissions ??
    app?.requestedPermissions ??
    null;

  return (
    <Dialog open={open} onOpenChange={(nextOpen) => !nextOpen && onCancel()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
        </DialogHeader>

        {app && manifest ? (
          <div className='space-y-5'>
            <div className='space-y-1 text-sm text-muted-foreground'>
              <div>{app.name}</div>
              <div>v{manifest.version}</div>
              {description ? <div>{description}</div> : null}
            </div>

            <div className='space-y-3'>
              <h3 className='text-sm font-medium'>Permissions</h3>

              <AppPermissions
                permissions={requestedPermissions}
                grantedPermissions={grantedPermissions}
                editable
                onGrantedPermissionsChange={onGrantedPermissionsChange}
              />
            </div>

            {manifest.network && manifest.network.whitelist && manifest.network.whitelist.length > 0 ? (
              <div className='space-y-2'>
                <div className='text-sm font-medium'>Network allowlist</div>

                <div className='space-y-2 rounded-md border p-3'>
                  {manifest.network.whitelist.map((entry) => (
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

          <Button onClick={onConfirm} disabled={submitting}>
            {submitting ? 'Saving...' : 'Save'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
