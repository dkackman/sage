import { Checkbox } from '@/components/ui/checkbox';
import type {
  SageGrantedPermissions,
  SageGrantedWalletPermissions,
  SageWalletPermissions,
} from '@/bindings';
import React from 'react';

interface Props {
  wanted: SageWalletPermissions;
  granted: SageGrantedWalletPermissions;
  onGrantedPermissionsChange: React.Dispatch<
    React.SetStateAction<SageGrantedPermissions>
  >;
}

export function WalletPermissionSection({
  wanted,
  granted,
  onGrantedPermissionsChange,
}: Props) {
  return (
    <div className='space-y-2'>
      <div className='text-sm font-medium'>Wallet</div>

      <div className='space-y-2 rounded-md border p-3'>
        <label className='flex items-center gap-3 text-sm'>
          <Checkbox
            checked={granted.sendXch}
            disabled={wanted.sendXch?.required ?? false}
            onCheckedChange={(checked) => {
              onGrantedPermissionsChange((prev) => {
                return {
                  ...prev,
                  wallet: {
                    ...prev.wallet,
                    sendXch: Boolean(checked),
                  },
                };
              });
            }}
          />
          <span>
            Send XCH
            {wanted.sendXch?.required ? ' (required)' : ''}
          </span>
        </label>

        <label className='flex items-center gap-3 text-sm'>
          <Checkbox
            checked={granted.sendXchAutoSubmit}
            disabled={wanted.sendXchAutoSubmit?.required ?? false}
            onCheckedChange={(checked) => {
              onGrantedPermissionsChange((prev) => {
                return {
                  ...prev,
                  wallet: {
                    ...prev.wallet,
                    sendXchAutoSubmit: Boolean(checked),
                  },
                };
              });
            }}
          />
          <span>
            Automatic XCH send
            {wanted.sendXchAutoSubmit?.required ? ' (required)' : ''}
          </span>
        </label>
      </div>
    </div>
  );
}
