import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { open } from '@tauri-apps/plugin-dialog';
import { useState } from 'react';

interface Props {
  onInstall: (zipPath: string) => Promise<void>;
}

export function InstallAppForm({ onInstall }: Props) {
  const [installing, setInstalling] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleInstall() {
    try {
      setError(null);

      const selected = await open({
        multiple: false,
        directory: false,
        filters: [
          {
            name: 'Zip Archive',
            extensions: ['zip'],
          },
        ],
      });

      if (!selected || Array.isArray(selected)) {
        return;
      }

      setInstalling(true);
      await onInstall(selected);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setInstalling(false);
    }
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>Install App</CardTitle>
      </CardHeader>
      <CardContent className='space-y-4'>
        <p className='text-sm text-muted-foreground'>
          Install a Sage app package from a zip file.
        </p>

        <Button onClick={handleInstall} disabled={installing}>
          {installing ? 'Installing...' : 'Choose .zip'}
        </Button>

        {error ? <div className='text-sm text-destructive'>{error}</div> : null}
      </CardContent>
    </Card>
  );
}

