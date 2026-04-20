import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import type { InstalledSageApp, SageGrantedPermissions } from '@/bindings';
import React from 'react';
import { PermissionsEditor } from '@/components/apps/permissions/PermissionsEditor';

interface Props {
  app: InstalledSageApp | null;
  open: boolean;
  title: string;
  description?: string | null;
  error: string | null;
  submitting: boolean;
  grantedPermissions: SageGrantedPermissions;
  onGrantedPermissionsChange: (next: SageGrantedPermissions) => void;
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

            <PermissionsEditor
              app={app}
              grantedPermissions={grantedPermissions}
              onGrantedPermissionsChange={onGrantedPermissionsChange}
            />

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
