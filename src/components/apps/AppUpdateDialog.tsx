import { useEffect, useMemo, useState } from 'react';
import type {
  InstalledSageApp,
  SageAppUrlPreview,
  SageGrantedPermissions,
  SageNetworkPermissionTarget,
  SageRequestedPermissions,
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

function sortStrings(values: Iterable<string>): string[] {
  return [...values].sort((a, b) => a.localeCompare(b));
}

function sortNetwork(
  values: Iterable<SageNetworkPermissionTarget>,
): SageNetworkPermissionTarget[] {
  return [...values].sort((a, b) => networkKey(a).localeCompare(networkKey(b)));
}

function buildAddedPermissionsDelta(
  previous: SageRequestedPermissions,
  next: SageRequestedPermissions,
): SageRequestedPermissions {
  const previousCaps = new Set([
    ...(previous.capabilities?.required ?? []),
    ...(previous.capabilities?.optional ?? []),
  ]);

  const previousNetwork = new Set([
    ...(previous.network?.whitelist?.required ?? []).map(networkKey),
    ...(previous.network?.whitelist?.optional ?? []).map(networkKey),
  ]);

  const addedRequiredCaps = (next.capabilities?.required ?? []).filter(
    (key) => !previousCaps.has(key),
  );
  const addedOptionalCaps = (next.capabilities?.optional ?? []).filter(
    (key) => !previousCaps.has(key),
  );

  const addedRequiredNetwork = (next.network?.whitelist?.required ?? []).filter(
    (entry) => !previousNetwork.has(networkKey(entry)),
  );
  const addedOptionalNetwork = (next.network?.whitelist?.optional ?? []).filter(
    (entry) => !previousNetwork.has(networkKey(entry)),
  );

  return {
    capabilities: {
      required: addedRequiredCaps,
      optional: addedOptionalCaps,
    },
    network: {
      whitelist: {
        required: addedRequiredNetwork,
        optional: addedOptionalNetwork,
      },
    },
  };
}

function buildRemovedGrantedDelta(
  app: InstalledSageApp,
  nextRequested: SageRequestedPermissions,
): SageGrantedPermissions {
  const nextAllowedCaps = new Set([
    ...(nextRequested.capabilities?.required ?? []),
    ...(nextRequested.capabilities?.optional ?? []),
  ]);

  const nextAllowedNetwork = new Set([
    ...(nextRequested.network?.whitelist?.required ?? []).map(networkKey),
    ...(nextRequested.network?.whitelist?.optional ?? []).map(networkKey),
  ]);

  const removedCapabilities = (
    app.grantedPermissions?.capabilities ?? []
  ).filter((key) => !nextAllowedCaps.has(key));

  const removedNetwork = (
    app.grantedPermissions?.network?.whitelist ?? []
  ).filter((entry) => !nextAllowedNetwork.has(networkKey(entry)));

  return {
    capabilities: sortStrings(removedCapabilities),
    network: {
      whitelist: sortNetwork(removedNetwork),
    },
  };
}

function buildFinalGrantedPermissions(args: {
  app: InstalledSageApp;
  nextRequested: SageRequestedPermissions;
  selectedAddedOptionalCapabilities: string[];
  selectedAddedOptionalNetwork: SageNetworkPermissionTarget[];
}): SageGrantedPermissions {
  const {
    app,
    nextRequested,
    selectedAddedOptionalCapabilities,
    selectedAddedOptionalNetwork,
  } = args;

  const nextRequiredCaps = new Set(nextRequested.capabilities?.required ?? []);
  const nextOptionalCaps = new Set(nextRequested.capabilities?.optional ?? []);
  const nextAllowedCaps = new Set([...nextRequiredCaps, ...nextOptionalCaps]);

  const retainedCapabilities = (
    app.grantedPermissions?.capabilities ?? []
  ).filter((key) => nextAllowedCaps.has(key));

  const finalCapabilities = sortStrings(
    new Set([
      ...retainedCapabilities,
      ...nextRequiredCaps,
      ...selectedAddedOptionalCapabilities,
    ]),
  );

  const nextRequiredNetwork = nextRequested.network?.whitelist?.required ?? [];
  const nextOptionalNetwork = nextRequested.network?.whitelist?.optional ?? [];
  const nextAllowedNetwork = new Set([
    ...nextRequiredNetwork.map(networkKey),
    ...nextOptionalNetwork.map(networkKey),
  ]);

  const retainedNetwork = (
    app.grantedPermissions?.network?.whitelist ?? []
  ).filter((entry) => nextAllowedNetwork.has(networkKey(entry)));

  const finalNetworkMap = new Map<string, SageNetworkPermissionTarget>();

  for (const entry of retainedNetwork) {
    finalNetworkMap.set(networkKey(entry), {
      scheme: entry.scheme,
      host: entry.host,
    });
  }

  for (const entry of nextRequiredNetwork) {
    finalNetworkMap.set(networkKey(entry), {
      scheme: entry.scheme,
      host: entry.host,
    });
  }

  for (const entry of selectedAddedOptionalNetwork) {
    finalNetworkMap.set(networkKey(entry), {
      scheme: entry.scheme,
      host: entry.host,
    });
  }

  return {
    capabilities: finalCapabilities,
    network: {
      whitelist: sortNetwork(finalNetworkMap.values()),
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

  const previousRequested = app?.requestedPermissions ?? null;
  const nextRequested = preview?.manifest.permissions ?? null;

  const addedDelta = useMemo(() => {
    if (!previousRequested || !nextRequested) {
      return null;
    }
    return buildAddedPermissionsDelta(previousRequested, nextRequested);
  }, [previousRequested, nextRequested]);

  const removedDelta = useMemo(() => {
    if (!app || !nextRequested) {
      return null;
    }
    return buildRemovedGrantedDelta(app, nextRequested);
  }, [app, nextRequested]);

  const finalGranted = useMemo(() => {
    if (!app || !nextRequested) {
      return null;
    }

    return buildFinalGrantedPermissions({
      app,
      nextRequested,
      selectedAddedOptionalCapabilities,
      selectedAddedOptionalNetwork,
    });
  }, [
    app,
    nextRequested,
    selectedAddedOptionalCapabilities,
    selectedAddedOptionalNetwork,
  ]);

  if (!app || !preview || !addedDelta || !removedDelta || !finalGranted) {
    return (
      <Dialog open={open} onOpenChange={(nextOpen) => !nextOpen && onCancel()}>
        <DialogContent />
      </Dialog>
    );
  }

  const addedRequiredNetwork = addedDelta.network?.whitelist?.required ?? [];
  const addedOptionalNetwork = addedDelta.network?.whitelist?.optional ?? [];
  const addedNetworkItems = [
    ...addedRequiredNetwork.map((entry) => ({ entry, required: true })),
    ...addedOptionalNetwork.map((entry) => ({ entry, required: false })),
  ];

  const removedCount =
    removedDelta.capabilities.length + removedDelta.network.whitelist.length;

  const addedCapabilityCount =
    (addedDelta.capabilities?.required?.length ?? 0) +
    (addedDelta.capabilities?.optional?.length ?? 0);

  const addedNetworkCount = addedNetworkItems.length;

  const selectedAddedOptionalNetworkKeys = new Set(
    selectedAddedOptionalNetwork.map(networkKey),
  );

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

          <div className='space-y-3'>
            <h3 className='text-sm font-medium'>New permissions requested</h3>

            {addedCapabilityCount === 0 && addedNetworkCount === 0 ? (
              <div className='text-sm text-muted-foreground'>
                This update does not add any new permissions.
              </div>
            ) : (
              <div className='space-y-4'>
                {addedCapabilityCount > 0 ? (
                  <div className='space-y-2'>
                    <div className='text-xs text-muted-foreground'>
                      Capabilities
                    </div>
                    <div className='rounded-md border p-3'>
                      <AppPermissions
                        permissions={addedDelta}
                        grantedPermissions={[
                          ...(addedDelta.capabilities?.required ?? []),
                          ...selectedAddedOptionalCapabilities,
                        ]}
                        editable
                        onGrantedPermissionsChange={(
                          nextSelectedAddedPermissions,
                        ) => {
                          const required = new Set(
                            addedDelta.capabilities?.required ?? [],
                          );

                          setSelectedAddedOptionalCapabilities(
                            nextSelectedAddedPermissions.filter(
                              (key) => !required.has(key),
                            ),
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
                                        prevMap.set(key, {
                                          scheme: entry.scheme,
                                          host: entry.host,
                                        });
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
            )}
          </div>

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
                {removedDelta.capabilities.length === 0 &&
                removedDelta.network.whitelist.length === 0 ? (
                  <div className='text-sm text-muted-foreground'>
                    No previously granted permissions are being removed.
                  </div>
                ) : (
                  <>
                    {removedDelta.capabilities.length > 0 ? (
                      <div className='space-y-2'>
                        <div className='text-xs text-muted-foreground'>
                          Previously granted capabilities that will be removed
                        </div>
                        <div className='space-y-1'>
                          {removedDelta.capabilities.map((key) => (
                            <div key={key} className='text-sm'>
                              {key}
                            </div>
                          ))}
                        </div>
                      </div>
                    ) : null}

                    {removedDelta.network.whitelist.length > 0 ? (
                      <div className='space-y-2'>
                        <div className='text-xs text-muted-foreground'>
                          Previously granted network access that will be removed
                        </div>
                        <div className='space-y-1'>
                          {removedDelta.network.whitelist.map((entry) => (
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
                  </>
                )}
              </div>
            ) : null}
          </div>

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
