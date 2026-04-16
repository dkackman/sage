import { Checkbox } from '@/components/ui/checkbox';
import type {
  SageGrantedNetworkPermissionEntry,
  SageGrantedPermissions,
  SageNetworkPermissionEntry,
} from '@/bindings';
import { isNetworkGranted } from './permissionUtils';
import React from 'react';

interface Props {
  wanted: SageNetworkPermissionEntry[];
  granted: SageGrantedNetworkPermissionEntry[];
  onGrantedPermissionsChange: React.Dispatch<
    React.SetStateAction<SageGrantedPermissions>
  >;
}

export function NetworkPermissionsSection({
  wanted,
  granted,
  onGrantedPermissionsChange,
}: Props) {
  if (wanted.length === 0) {
    return null;
  }

  function grantNetworkPermission(entry: SageNetworkPermissionEntry) {
    onGrantedPermissionsChange((prev) => {
      const alreadyGranted = prev.network.some(
        (g) => g.scheme === entry.scheme && g.host === entry.host,
      );

      if (alreadyGranted) {
        return prev;
      }

      return {
        ...prev,
        network: [
          ...prev.network,
          {
            scheme: entry.scheme,
            host: entry.host,
          },
        ],
      };
    });
  }

  function revokeNetworkPermission(entry: SageNetworkPermissionEntry) {
    onGrantedPermissionsChange((prev) => ({
      ...prev,
      network: prev.network.filter(
        (g) => !(g.scheme === entry.scheme && g.host === entry.host),
      ),
    }));
  }

  return (
    <div className='space-y-2'>
      <div className='text-sm font-medium'>Network allowlist</div>

      <div className='space-y-2 rounded-md border p-3'>
        {wanted.map((entry) => {
          const url = `${entry.scheme}://${entry.host}`;
          const isGranted = isNetworkGranted(entry, granted);

          return (
            <label key={url} className='flex items-center gap-3 text-sm'>
              <Checkbox
                checked={isGranted}
                disabled={entry.required}
                onCheckedChange={(nextChecked) =>
                  nextChecked
                    ? grantNetworkPermission(entry)
                    : revokeNetworkPermission(entry)
                }
              />
              <span className='font-mono text-xs'>
                {url}
                {entry.required ? ' (required)' : ''}
              </span>
            </label>
          );
        })}
      </div>
    </div>
  );
}
