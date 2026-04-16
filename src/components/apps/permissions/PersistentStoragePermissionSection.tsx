import { Checkbox } from '@/components/ui/checkbox';
import type {
  SageGrantedPermissions,
  SagePermissionRequired,
} from '@/bindings';
import React from 'react';

interface Props {
  wanted: SagePermissionRequired;
  granted: boolean;
  onGrantedPermissionsChange: React.Dispatch<
    React.SetStateAction<SageGrantedPermissions>
  >;
}

export function PersistentStoragePermissionSection({
  wanted,
  granted,
  onGrantedPermissionsChange,
}: Props) {
  function setGranted(granted: boolean) {
    onGrantedPermissionsChange((prev) => {
      return {
        ...prev,
        persistentStorage: granted,
      };
    });
  }

  return (
    <label className='flex items-center gap-3 text-sm'>
      <Checkbox
        checked={granted}
        disabled={wanted.required ?? false}
        onCheckedChange={(checked) => {
          setGranted(Boolean(checked));
        }}
      />
      <span>
        Persistent storage
        {wanted.required ? ' (required)' : ''}
      </span>
    </label>
  );
}
