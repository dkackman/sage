import { useEffect, useMemo, useState } from 'react';
import type {
  InstalledSageApp,
  SageAppUrlPreview,
  SageGrantedPermissions,
  SageNetworkPermissionTarget,
} from '@/bindings';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { AppPermissions } from '@/components/apps/permissions/AppPermissions.tsx';
import {
  getAppUpdatePermissionsDelta,
  type AppUpdatePermissionsDelta,
} from '@/lib/apps/updatePermissionsDelta';

interface Props {
  open: boolean;
  app: InstalledSageApp | null;
  preview: SageAppUrlPreview | null;
  submitting: boolean;
  error: string | null;
  onCancel: () => void;
  onConfirm: (nextGranted: SageGrantedPermissions) => void;
}

function networkKey(entry: SageNetworkPermissionTarget): string {
  return `${entry.scheme}://${entry.host}`;
}

function sortNetwork(
  values: Iterable<SageNetworkPermissionTarget>,
): SageNetworkPermissionTarget[] {
  return [...values].sort((a, b) => networkKey(a).localeCompare(networkKey(b)));
}

function buildAddedPermissionsViewModel(
  delta: AppUpdatePermissionsDelta,
): InstalledSageApp['requestedPermissions'] {
  return {
    capabilities: {
      required: delta.addedRequestedCapabilities.required,
      optional: delta.addedRequestedCapabilities.optional,
    },
    network: {
      whitelist: {
        required: [],
        optional: [],
      },
    },
  };
}

export function AppUpdateDialog({
  open,
  app,
  preview,
  submitting,
  error,
  onCancel,
  onConfirm,
}: Props) {
  const [showRemoved, setShowRemoved] = useState(false);
  const [
    selectedAddedOptionalCapabilities,
    setSelectedAddedOptionalCapabilities,
  ] = useState<string[]>([]);
  const [selectedAddedOptionalNetwork, setSelectedAddedOptionalNetwork] =
    useState<SageNetworkPermissionTarget[]>([]);

  useEffect(() => {
    if (!open) {
      setShowRemoved(false);
      setSelectedAddedOptionalCapabilities([]);
      setSelectedAddedOptionalNetwork([]);
    }
  }, [open]);

  const delta = useMemo(() => {
    if (!app || !preview) {
      return null;
    }

    return getAppUpdatePermissionsDelta(app, preview);
  }, [app, preview]);

  const finalGranted = useMemo(() => {
    if (!delta) {
      return null;
    }

    const selectedOptionalCapabilitySet = new Set(
      selectedAddedOptionalCapabilities,
    );
    const selectedOptionalNetworkSet = new Set(
      selectedAddedOptionalNetwork.map(networkKey),
    );

    const nextCapabilities = [
      ...delta.nextGrantedPermissions.capabilities,
      ...delta.addedRequestedCapabilities.optional.filter((key) =>
        selectedOptionalCapabilitySet.has(key),
      ),
    ].sort((a, b) => a.localeCompare(b));

    const nextNetworkMap = new Map<string, SageNetworkPermissionTarget>(
      delta.nextGrantedPermissions.network.whitelist.map((item) => [
        networkKey(item),
        item,
      ]),
    );

    for (const item of delta.addedRequestedNetwork.optional) {
      if (selectedOptionalNetworkSet.has(networkKey(item))) {
        nextNetworkMap.set(networkKey(item), item);
      }
    }

    for (const item of delta.addedRequestedNetwork.required) {
      nextNetworkMap.set(networkKey(item), item);
    }

    return {
      capabilities: nextCapabilities,
      network: {
        whitelist: sortNetwork(nextNetworkMap.values()),
      },
    } satisfies SageGrantedPermissions;
  }, [delta, selectedAddedOptionalCapabilities, selectedAddedOptionalNetwork]);

  if (!app || !preview || !delta || !finalGranted) {
    return (
      <Dialog open={open} onOpenChange={(nextOpen) => !nextOpen && onCancel()}>
        <DialogContent />
      </Dialog>
    );
  }

  const addedRequiredCapabilitiesCount =
    delta.addedRequestedCapabilities.required.length;
  const addedOptionalCapabilitiesCount =
    delta.addedRequestedCapabilities.optional.length;
  const addedCapabilityCount =
    addedRequiredCapabilitiesCount + addedOptionalCapabilitiesCount;

  const addedRequiredNetworkCount = delta.addedRequestedNetwork.required.length;
  const addedOptionalNetworkCount = delta.addedRequestedNetwork.optional.length;
  const addedNetworkCount =
    addedRequiredNetworkCount + addedOptionalNetworkCount;

  const removedCount =
    delta.removedGrantedCapabilities.length +
    delta.removedGrantedNetwork.length;

  const selectedAddedOptionalNetworkKeys = new Set(
    selectedAddedOptionalNetwork.map(networkKey),
  );

  const addedNetworkItems = [
    ...delta.addedRequestedNetwork.required.map((entry) => ({
      entry,
      required: true,
    })),
    ...delta.addedRequestedNetwork.optional.map((entry) => ({
      entry,
      required: false,
    })),
  ];

  return (
    <Dialog open={open} onOpenChange={(nextOpen) => !nextOpen && onCancel()}>
      <DialogContent className='max-w-2xl'>
        <DialogHeader>
          <DialogTitle>Review app update</DialogTitle>
        </DialogHeader>

        <div className='space-y-5'>
          <div className='space-y-1 text-sm text-muted-foreground'>
            <div>{app.name}</div>
            <div>
              v{app.version} → v{preview.manifest.version}
            </div>
          </div>

          {addedCapabilityCount > 0 || addedNetworkCount > 0 ? (
            <div className='space-y-3'>
              <h3 className='text-sm font-medium'>New permissions requested</h3>

              <div className='space-y-4'>
                {addedCapabilityCount > 0 ? (
                  <div className='space-y-2'>
                    <div className='text-xs text-muted-foreground'>
                      Capabilities
                    </div>

                    <div className='rounded-md border p-3'>
                      <AppPermissions
                        permissions={buildAddedPermissionsViewModel(delta)}
                        grantedPermissions={[
                          ...delta.addedRequestedCapabilities.required,
                          ...selectedAddedOptionalCapabilities,
                        ]}
                        editable
                        onGrantedPermissionsChange={(next) => {
                          const requiredSet = new Set(
                            delta.addedRequestedCapabilities.required,
                          );

                          setSelectedAddedOptionalCapabilities(
                            next.filter((key) => !requiredSet.has(key)),
                          );
                        }}
                      />
                    </div>
                  </div>
                ) : null}

                {addedNetworkCount > 0 ? (
                  <div className='space-y-2'>
                    <div className='text-xs text-muted-foreground'>
                      Network access
                    </div>

                    <div className='space-y-2 rounded-md border p-3'>
                      {addedNetworkItems.map(({ entry, required }) => {
                        const key = networkKey(entry);
                        const checked =
                          required || selectedAddedOptionalNetworkKeys.has(key);

                        return (
                          <label
                            key={key}
                            className='flex items-center justify-between gap-3 text-xs'
                          >
                            <div className='min-w-0 font-mono break-all'>
                              {key}
                            </div>

                            <div className='shrink-0'>
                              {required ? (
                                <span className='text-muted-foreground'>
                                  required
                                </span>
                              ) : (
                                <input
                                  type='checkbox'
                                  checked={checked}
                                  onChange={(event) => {
                                    const nextChecked = event.target.checked;

                                    setSelectedAddedOptionalNetwork((prev) => {
                                      const prevMap = new Map(
                                        prev.map((item) => [
                                          networkKey(item),
                                          item,
                                        ]),
                                      );

                                      if (nextChecked) {
                                        prevMap.set(key, entry);
                                      } else {
                                        prevMap.delete(key);
                                      }

                                      return sortNetwork(prevMap.values());
                                    });
                                  }}
                                />
                              )}
                            </div>
                          </label>
                        );
                      })}
                    </div>
                  </div>
                ) : null}
              </div>
            </div>
          ) : null}

          {removedCount > 0 ? (
            <div className='space-y-2'>
              <button
                type='button'
                className='text-left text-sm font-medium underline-offset-4 hover:underline'
                onClick={() => {
                  setShowRemoved((prev) => !prev);
                }}
              >
                Removed permissions ({removedCount})
                {showRemoved ? ' — hide details' : ' — show details'}
              </button>

              {showRemoved ? (
                <div className='space-y-4 rounded-md border p-3'>
                  {delta.removedGrantedCapabilities.length > 0 ? (
                    <div className='space-y-2'>
                      <div className='text-xs text-muted-foreground'>
                        Previously granted capabilities that will be removed
                      </div>

                      <div className='space-y-1'>
                        {delta.removedGrantedCapabilities.map((key) => (
                          <div key={key} className='text-sm'>
                            {key}
                          </div>
                        ))}
                      </div>
                    </div>
                  ) : null}

                  {delta.removedGrantedNetwork.length > 0 ? (
                    <div className='space-y-2'>
                      <div className='text-xs text-muted-foreground'>
                        Previously granted network access that will be removed
                      </div>

                      <div className='space-y-1'>
                        {delta.removedGrantedNetwork.map((entry) => (
                          <div
                            key={networkKey(entry)}
                            className='font-mono text-sm break-all'
                          >
                            {networkKey(entry)}
                          </div>
                        ))}
                      </div>
                    </div>
                  ) : null}
                </div>
              ) : null}
            </div>
          ) : null}

          {error ? (
            <div className='text-sm text-destructive'>{error}</div>
          ) : null}
        </div>

        <DialogFooter>
          <Button variant='outline' onClick={onCancel} disabled={submitting}>
            Cancel
          </Button>

          <Button
            onClick={() => {
              onConfirm(finalGranted);
            }}
            disabled={submitting}
          >
            {submitting ? 'Updating...' : 'Confirm update'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
