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
import { Input } from '@/components/ui/input';
import { open } from '@tauri-apps/plugin-dialog';
import { useMemo, useState } from 'react';
import type {
  SageAppPackageManifest,
  SageAppUrlPreview,
  SageGrantedNetworkPermissionEntry,
  SageGrantedPermissions,
  SageNetworkPermissionEntry,
} from '@/bindings';
import { formatAppError } from '@/lib/apps/formatAppError.ts';

interface Props {
  onPreviewZip: (zipPath: string) => Promise<SageAppPackageManifest>;
  onPreviewUrl: (appUrl: string) => Promise<SageAppUrlPreview>;
  onInstallZip: (
    zipPath: string,
    permissions: SageGrantedPermissions,
  ) => Promise<void>;
  onInstallUrl: (
    appUrl: string,
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

type InstallSource =
  | {
      kind: 'zip';
      zipPath: string;
      manifest: SageAppPackageManifest;
    }
  | {
      kind: 'url';
      appUrl: string;
      preview: SageAppUrlPreview;
    };

export function InstallAppForm({
  onPreviewZip,
  onPreviewUrl,
  onInstallZip,
  onInstallUrl,
}: Props) {
  const [installing, setInstalling] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [urlInput, setUrlInput] = useState('');

  const [source, setSource] = useState<InstallSource | null>(null);

  const manifest = useMemo(() => {
    if (!source) return null;
    return source.kind === 'zip' ? source.manifest : source.preview.manifest;
  }, [source]);

  const [grantedNetworkKeys, setGrantedNetworkKeys] = useState<string[]>([]);
  const [persistentStorageGranted, setPersistentStorageGranted] =
    useState(false);

  const sortedNetworkEntries = useMemo(() => {
    return manifest
      ? sortNetworkEntries(manifest.permissions.network ?? [])
      : [];
  }, [manifest]);

  async function handleSelectZip() {
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

      const nextManifest = await onPreviewZip(selected);

      const network = nextManifest.permissions.network ?? [];
      const persistentStorage =
        nextManifest.permissions.persistent_storage ?? null;

      setSource({
        kind: 'zip',
        zipPath: selected,
        manifest: {
          ...nextManifest,
          permissions: {
            ...nextManifest.permissions,
            network,
            persistent_storage: persistentStorage,
          },
        },
      });

      setGrantedNetworkKeys(network.map((entry) => networkEntryKey(entry)));
      setPersistentStorageGranted(
        !!nextManifest.permissions.persistent_storage,
      );
    } catch (err) {
      setError(formatAppError(err));
    }
  }

  async function handlePreviewUrl() {
    try {
      setError(null);

      const preview = await onPreviewUrl(urlInput.trim());
      const network = preview.manifest.permissions.network ?? [];
      const persistentStorage =
        preview.manifest.permissions.persistent_storage ?? null;

      setSource({
        kind: 'url',
        appUrl: preview.appUrl,
        preview: {
          ...preview,
          manifest: {
            ...preview.manifest,
            permissions: {
              ...preview.manifest.permissions,
              network,
              persistent_storage: persistentStorage,
            },
          },
        },
      });

      setGrantedNetworkKeys(network.map((entry) => networkEntryKey(entry)));
      setPersistentStorageGranted(
        !!preview.manifest.permissions.persistent_storage,
      );
    } catch (err) {
      setError(formatAppError(err));
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
    if (!source || !manifest) {
      return;
    }

    try {
      setInstalling(true);
      setError(null);

      const permissions = buildGrantedPermissions();

      if (source.kind === 'zip') {
        await onInstallZip(source.zipPath, permissions);
      } else {
        await onInstallUrl(source.appUrl, permissions);
      }

      setSource(null);
      setGrantedNetworkKeys([]);
      setPersistentStorageGranted(false);
      setUrlInput('');
    } catch (err) {
      setError(formatAppError(err));
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
            Install a Sage app package from a zip file or URL.
          </p>

          <div className='flex flex-wrap gap-2'>
            <Button onClick={handleSelectZip} disabled={installing}>
              Install from .zip
            </Button>
          </div>

          <div className='space-y-2 rounded-md border p-3'>
            <div className='text-sm font-medium'>Install from URL</div>
            <div className='flex gap-2'>
              <Input
                value={urlInput}
                onChange={(e) => setUrlInput(e.target.value)}
                placeholder='https://example.com/my-app/'
                disabled={installing}
              />
              <Button
                onClick={handlePreviewUrl}
                disabled={installing || urlInput.trim().length === 0}
              >
                Preview URL
              </Button>
            </div>
          </div>

          {error ? (
            <div className='text-sm text-destructive'>{error}</div>
          ) : null}
        </CardContent>
      </Card>

      <Dialog
        open={!!manifest}
        onOpenChange={(open) => {
          if (!open) {
            setSource(null);
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
            <div className='space-y-1 text-sm text-muted-foreground'>
              <div>v{manifest?.version}</div>
              {source?.kind === 'url' ? (
                <>
                  <div className='break-all'>URL: {source.preview.appUrl}</div>
                  <div className='break-all'>
                    Manifest: {source.preview.manifestUrl}
                  </div>
                </>
              ) : null}
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
                setSource(null);
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
