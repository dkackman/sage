import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Checkbox } from '@/components/ui/checkbox';
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { useMemo, useState } from 'react';
import type {
  SageAppPackageManifest,
  SageGrantedNetworkPermissionEntry,
  SageGrantedPermissions,
  SageNetworkPermissionEntry,
} from '@/bindings';

interface Props {
  onInstall: (
    zipPath: string,
    permissions: SageGrantedPermissions,
  ) => Promise<void>;
}

function networkEntryKey(entry: { scheme: string; host: string }): string {
  return `${entry.scheme}://${entry.host}`;
}

function sortNetworkEntries(entries: SageNetworkPermissionEntry[]) {
  return [...entries].sort((a, b) => {
    const aKey = `${a.scheme}://${a.host}`;
    const bKey = `${b.scheme}://${b.host}`;
    return aKey.localeCompare(bKey);
  });
}

export function InstallAppForm({ onInstall }: Props) {
  const [installing, setInstalling] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [manifest, setManifest] = useState<SageAppPackageManifest | null>(null);
  const [zipPath, setZipPath] = useState<string | null>(null);

  const [grantedNetworkKeys, setGrantedNetworkKeys] = useState<string[]>([]);
  const [persistentStorageGranted, setPersistentStorageGranted] =
    useState(false);

  const sortedNetworkEntries = useMemo(() => {
    return manifest
      ? sortNetworkEntries(manifest.permissions.network ?? [])
      : [];
  }, [manifest]);

  async function handleSelect() {
    try {
      setError(null);

      const selected = await open({
        multiple: false,
        directory: false,
        filters: [{ name: 'Zip Archive', extensions: ['zip'] }],
      });

      if (!selected || Array.isArray(selected)) {
        return;
      }

      const nextManifest = await invoke<SageAppPackageManifest>(
        'preview_app_zip',
        {
          zipPath: selected,
        },
      );

      const network = nextManifest.permissions.network ?? [];
      const persistentStorage =
        nextManifest.permissions.persistent_storage ?? null;

      setManifest({
        ...nextManifest,
        permissions: {
          ...nextManifest.permissions,
          network,
          persistent_storage: persistentStorage,
        },
      });
      setZipPath(selected);

      setGrantedNetworkKeys(network.map((entry) => networkEntryKey(entry)));

      setPersistentStorageGranted(!!nextManifest.permissions.persistent_storage);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  function buildGrantedPermissions(): SageGrantedPermissions {
    const grantedNetwork: SageGrantedNetworkPermissionEntry[] =
      sortedNetworkEntries
        .filter((entry) => grantedNetworkKeys.includes(networkEntryKey(entry)))
        .map((entry) => ({
          scheme: entry.scheme,
          host: entry.host,
        }));

    return {
      network: grantedNetwork,
      persistentStorage: persistentStorageGranted,
    };
  }

  async function confirmInstall() {
    if (!zipPath || !manifest) {
      return;
    }

    try {
      setInstalling(true);
      setError(null);

      await onInstall(zipPath, buildGrantedPermissions());

      setManifest(null);
      setZipPath(null);
      setGrantedNetworkKeys([]);
      setPersistentStorageGranted(false);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setInstalling(false);
    }
  }

  return (
    <>
      <Card>
        <CardHeader>
          <CardTitle>Install App</CardTitle>
        </CardHeader>
        <CardContent className='space-y-4'>
          <p className='text-sm text-muted-foreground'>
            Install a Sage app package from a zip file.
          </p>

          <Button onClick={handleSelect} disabled={installing}>
            Choose .zip
          </Button>

          {error ? (
            <div className='text-sm text-destructive'>{error}</div>
          ) : null}
        </CardContent>
      </Card>

      <Dialog
        open={!!manifest}
        onOpenChange={(open) => {
          if (!open) {
            setManifest(null);
            setZipPath(null);
            setGrantedNetworkKeys([]);
            setPersistentStorageGranted(false);
          }
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Install {manifest?.name}</DialogTitle>
          </DialogHeader>

          <div className='space-y-5'>
            <div className='text-sm text-muted-foreground'>
              v{manifest?.version}
            </div>

            <div className='space-y-3'>
              <h3 className='text-sm font-medium'>Permissions</h3>

              {manifest?.permissions.persistent_storage ? (
                <label className='flex items-center gap-3 text-sm'>
                  <Checkbox
                    checked={persistentStorageGranted}
                    disabled={manifest.permissions.persistent_storage.required}
                    onCheckedChange={(checked) => {
                      setPersistentStorageGranted(Boolean(checked));
                    }}
                  />
                  <span>
                    Persistent storage
                    {manifest.permissions.persistent_storage.required
                      ? ' (required)'
                      : ''}
                  </span>
                </label>
              ) : null}

              {sortedNetworkEntries.length > 0 ? (
                <div className='space-y-2'>
                  <div className='text-sm font-medium'>Network allowlist</div>

                  <div className='space-y-2 rounded-md border p-3'>
                    {sortedNetworkEntries.map((entry) => {
                      const key = networkEntryKey(entry);
                      const checked = grantedNetworkKeys.includes(key);

                      return (
                        <label
                          key={key}
                          className='flex items-center gap-3 text-sm'
                        >
                          <Checkbox
                            checked={checked}
                            disabled={entry.required}
                            onCheckedChange={(nextChecked) => {
                              setGrantedNetworkKeys((prev) => {
                                if (nextChecked) {
                                  if (prev.includes(key)) {
                                    return prev;
                                  }
                                  return [...prev, key];
                                }

                                return prev.filter((value) => value !== key);
                              });
                            }}
                          />
                          <span className='font-mono text-xs'>
                            {entry.scheme}://{entry.host}
                            {entry.required ? ' (required)' : ''}
                          </span>
                        </label>
                      );
                    })}
                  </div>
                </div>
              ) : null}

              {!manifest?.permissions.persistent_storage &&
              sortedNetworkEntries.length === 0 ? (
                <div className='text-sm text-muted-foreground'>
                  This app does not request any permissions.
                </div>
              ) : null}
            </div>

            {error ? (
              <div className='text-sm text-destructive'>{error}</div>
            ) : null}
          </div>

          <DialogFooter>
            <Button
              variant='outline'
              onClick={() => {
                setManifest(null);
                setZipPath(null);
                setGrantedNetworkKeys([]);
                setPersistentStorageGranted(false);
              }}
            >
              Cancel
            </Button>

            <Button onClick={confirmInstall} disabled={installing}>
              {installing ? 'Installing...' : 'Install'}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}
