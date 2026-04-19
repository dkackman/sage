import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import type { InstalledSageApp, SageNetworkPermissionTarget } from '@/bindings';
import React from 'react';
import { PermissionsEditor } from '@/components/apps/permissions/PermissionsEditor';

interface Props {
  app: InstalledSageApp | null;
  open: boolean;
  title: string;
  description?: string | null;
  error: string | null;
  submitting: boolean;
  grantedCapabilities: string[];
  grantedNetworkWhitelist: SageNetworkPermissionTarget[];
  onGrantedCapabilitiesChange: (next: string[]) => void;
  onGrantedNetworkWhitelistChange: (
    next: SageNetworkPermissionTarget[],
  ) => void;
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
  grantedCapabilities,
  grantedNetworkWhitelist,
  onGrantedCapabilitiesChange,
  onGrantedNetworkWhitelistChange,
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
              grantedCapabilities={grantedCapabilities}
              grantedNetworkWhitelist={grantedNetworkWhitelist}
              onGrantedCapabilitiesChange={onGrantedCapabilitiesChange}
              onGrantedNetworkWhitelistChange={onGrantedNetworkWhitelistChange}
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
