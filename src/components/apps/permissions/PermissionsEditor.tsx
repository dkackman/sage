import type { InstalledSageApp, SageNetworkPermissionTarget } from '@/bindings';
import { AppPermissions } from './AppPermissions';

interface Props {
  app: InstalledSageApp;
  grantedPermissions: string[];
  grantedNetworkWhitelist: SageNetworkPermissionTarget[];
  onGrantedPermissionsChange: (next: string[]) => void;
  onGrantedNetworkWhitelistChange: (
    next: SageNetworkPermissionTarget[],
  ) => void;
}

function sortNetworkEntries(
  entries: SageNetworkPermissionTarget[],
): SageNetworkPermissionTarget[] {
  return [...entries].sort((a, b) => {
    const aKey = `${a.scheme}://${a.host}`;
    const bKey = `${b.scheme}://${b.host}`;
    return aKey.localeCompare(bKey);
  });
}

function networkKey(entry: SageNetworkPermissionTarget): string {
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

  const requestedRequiredNetwork =
    manifest.permissions?.network?.whitelist?.required ?? [];
  const requestedOptionalNetwork =
    manifest.permissions?.network?.whitelist?.optional ?? [];
  const requestedNetwork = [
    ...requestedRequiredNetwork.map((entry) => ({
      entry,
      required: true,
    })),
    ...requestedOptionalNetwork.map((entry) => ({
      entry,
      required: false,
    })),
  ];

  const grantedNetworkKeys = new Set(
    grantedNetworkWhitelist.map((entry) => networkKey(entry)),
  );

  function handleToggleNetwork(
    entry: SageNetworkPermissionTarget,
    nextGranted: boolean,
  ) {
    const requiredEntries = requestedRequiredNetwork;
    const optionalEntries = requestedOptionalNetwork;

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
            {requestedNetwork.map(({ entry, required }) => {
              const key = networkKey(entry);
              const checked = required || grantedNetworkKeys.has(key);

              return (
                <label
                  key={key}
                  className='flex items-center justify-between gap-3 text-xs'
                >
                  <div className='min-w-0 font-mono break-all'>{key}</div>

                  <div className='shrink-0'>
                    {required ? (
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
