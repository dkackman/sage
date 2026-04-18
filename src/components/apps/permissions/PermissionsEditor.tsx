import type { InstalledSageApp, SageNetworkWhitelistEntry } from '@/bindings';
import { AppPermissions } from './AppPermissions';

interface Props {
  app: InstalledSageApp;
  grantedPermissions: string[];
  grantedNetworkWhitelist: SageNetworkWhitelistEntry[];
  onGrantedPermissionsChange: (next: string[]) => void;
  onGrantedNetworkWhitelistChange: (next: SageNetworkWhitelistEntry[]) => void;
}

function sortNetworkEntries(
  entries: SageNetworkWhitelistEntry[],
): SageNetworkWhitelistEntry[] {
  return [...entries].sort((a, b) => {
    const aKey = `${a.scheme}://${a.host}`;
    const bKey = `${b.scheme}://${b.host}`;
    return aKey.localeCompare(bKey);
  });
}

function networkKey(entry: SageNetworkWhitelistEntry): string {
  return `${entry.scheme}://${entry.host}`;
}

export function PermissionsEditor({
  app,
  grantedPermissions,
  grantedNetworkWhitelist,
  onGrantedPermissionsChange,
  onGrantedNetworkWhitelistChange,
}: Props) {
  const manifest = app.pendingUpdate?.manifest ?? app.activeSnapshot.manifest;
  const requestedNetwork = manifest.network?.whitelist ?? [];

  const grantedNetworkKeys = new Set(
    grantedNetworkWhitelist.map((entry) => networkKey(entry)),
  );

  function handleToggleNetwork(
    entry: SageNetworkWhitelistEntry,
    nextGranted: boolean,
  ) {
    const requiredEntries = requestedNetwork.filter((item) => item.required);
    const optionalEntries = requestedNetwork.filter((item) => !item.required);

    const nextOptional = optionalEntries.filter((item) => {
      const key = networkKey(item);

      if (key !== networkKey(entry)) {
        return grantedNetworkKeys.has(key);
      }

      return nextGranted;
    });

    onGrantedNetworkWhitelistChange(
      sortNetworkEntries([...requiredEntries, ...nextOptional]),
    );
  }

  return (
    <div className='space-y-5'>
      <div className='space-y-3'>
        <h3 className='text-sm font-medium'>Permissions</h3>

        <AppPermissions
          permissions={app.requestedPermissions}
          grantedPermissions={grantedPermissions}
          editable
          onGrantedPermissionsChange={onGrantedPermissionsChange}
        />
      </div>

      {requestedNetwork.length > 0 ? (
        <div className='space-y-3'>
          <h3 className='text-sm font-medium'>Network access</h3>

          <div className='space-y-2 rounded-md border p-3'>
            {requestedNetwork.map((entry) => {
              const key = networkKey(entry);
              const checked = entry.required || grantedNetworkKeys.has(key);

              return (
                <label
                  key={key}
                  className='flex items-center justify-between gap-3 text-xs'
                >
                  <div className='min-w-0 font-mono break-all'>{key}</div>

                  <div className='shrink-0'>
                    {entry.required ? (
                      <span className='text-muted-foreground'>required</span>
                    ) : (
                      <input
                        type='checkbox'
                        checked={checked}
                        onChange={(event) => {
                          handleToggleNetwork(entry, event.target.checked);
                        }}
                      />
                    )}
                  </div>
                </label>
              );
            })}
          </div>

          <div className='text-xs text-muted-foreground'>
            Network access is configured separately from permissions.
          </div>
        </div>
      ) : null}
    </div>
  );
}
