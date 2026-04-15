import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Checkbox } from '@/components/ui/checkbox';
import { open } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';
import { useState } from 'react';
import type { SageAppPackageManifest, SageAppPermissions } from '@/bindings';

interface Props {
  onInstall: (
    zipPath: string,
    permissions: SageAppPermissions,
  ) => Promise<void>;
}

export function InstallAppForm({ onInstall }: Props) {
  const [installing, setInstalling] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [manifest, setManifest] = useState<SageAppPackageManifest | null>(null);
  const [zipPath, setZipPath] = useState<string | null>(null);
  const [permissions, setPermissions] = useState<SageAppPermissions>({
    network: false,
    persistentStorage: false,
  });

  async function handleSelect() {
    try {
      setError(null);

      const selected = await open({
        multiple: false,
        directory: false,
        filters: [{ name: 'Zip Archive', extensions: ['zip'] }],
      });

      if (!selected || Array.isArray(selected)) return;

      const manifest = await invoke<SageAppPackageManifest>('preview_app_zip', {
        zipPath: selected,
      });

      setManifest(manifest);
      setZipPath(selected);

      setPermissions({
        network: !!manifest.permissions.network,
        persistentStorage: !!manifest.permissions.persistentStorage,
      });
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function confirmInstall() {
    if (!zipPath || !manifest) return;

    try {
      setInstalling(true);

      await onInstall(zipPath, permissions);

      setManifest(null);
      setZipPath(null);
      setPermissions({
        network: false,
        persistentStorage: false,
      });
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setInstalling(false);
    }
  }

  const required = new Set(manifest?.required_permissions ?? []);

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

      <Dialog open={!!manifest} onOpenChange={() => setManifest(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Install {manifest?.name}</DialogTitle>
          </DialogHeader>

          <div className='space-y-4'>
            <div className='text-sm text-muted-foreground'>
              v{manifest?.version}
            </div>

            <div className='space-y-2'>
              {manifest &&
                (
                  Object.entries(manifest.permissions) as [
                    keyof SageAppPermissions,
                    boolean,
                  ][]
                ).map(([key, value]) => {
                  if (!value) return null;

                  const isRequired = required.has(key);

                  return (
                    <label
                      key={key}
                      className='flex items-center gap-3 text-sm'
                    >
                      <Checkbox
                        checked={permissions[key]}
                        disabled={isRequired}
                        onCheckedChange={(checked) => {
                          setPermissions((prev) => ({
                            ...prev,
                            [key]: Boolean(checked),
                          }));
                        }}
                      />

                      <span>
                        {key === 'persistentStorage'
                          ? 'persistent storage'
                          : key}
                        {isRequired ? ' (required)' : ''}
                      </span>
                    </label>
                  );
                })}
            </div>
          </div>

          <DialogFooter>
            <Button variant='outline' onClick={() => setManifest(null)}>
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
