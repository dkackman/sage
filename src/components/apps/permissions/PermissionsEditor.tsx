import { useEffect, useState } from 'react';
import type { InstalledSageApp } from '@/bindings';
import { Button } from '@/components/ui/button';
import { AppPermissions } from './AppPermissions';

interface Props {
  app: InstalledSageApp;
  onCancel: () => void;
  onApply: (permissions: string[]) => void;
}

export function PermissionsEditor({ app, onCancel, onApply }: Props) {
  const [grantedPermissions, setGrantedPermissions] = useState<string[]>(
    app.grantedPermissions ?? [],
  );

  useEffect(() => {
    setGrantedPermissions(app.grantedPermissions ?? []);
  }, [app]);

  return (
    <div className='space-y-4'>
      <AppPermissions
        permissions={app.requestedPermissions}
        grantedPermissions={grantedPermissions}
        editable
        onGrantedPermissionsChange={setGrantedPermissions}
      />

      <div className='flex justify-end gap-2'>
        <Button variant='outline' onClick={onCancel}>
          Cancel
        </Button>

        <Button
          onClick={() => {
            onApply(grantedPermissions);
          }}
        >
          Apply
        </Button>
      </div>
    </div>
  );
}
