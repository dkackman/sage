import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { open } from '@tauri-apps/plugin-dialog';

interface Props {
  installing: boolean;
  urlInput: string;
  onUrlInputChange: (value: string) => void;
  onSelectZipPath: (zipPath: string) => Promise<void>;
  onPreviewUrl: () => Promise<void>;
  error: string | null;
}

export function InstallSourceCard({
  installing,
  urlInput,
  onUrlInputChange,
  onSelectZipPath,
  onPreviewUrl,
  error,
}: Props) {
  async function handleSelectZip() {
    const selected = await open({
      multiple: false,
      directory: false,
      filters: [{ name: 'Zip Archive', extensions: ['zip'] }],
    });

    if (!selected || Array.isArray(selected)) {
      return;
    }

    await onSelectZipPath(selected);
  }

  return (
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
              onChange={(e) => onUrlInputChange(e.target.value)}
              placeholder='https://example.com/my-app/'
              disabled={installing}
            />
            <Button
              onClick={() => void onPreviewUrl()}
              disabled={installing || urlInput.trim().length === 0}
            >
              Preview URL
            </Button>
          </div>
        </div>

        {error ? <div className='text-sm text-destructive'>{error}</div> : null}
      </CardContent>
    </Card>
  );
}

