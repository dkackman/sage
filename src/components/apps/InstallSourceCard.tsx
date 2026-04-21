import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { open } from '@tauri-apps/plugin-dialog';
import { Globe, Link as LinkIcon, Package, Upload } from 'lucide-react';

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
      <CardContent className='space-y-5'>
        <div className='rounded-xl border bg-muted/20 p-4'>
          <div className='mb-3 flex items-start gap-3'>
            <div className='mt-0.5 rounded-lg border bg-background p-2 text-muted-foreground'>
              <Globe className='h-4 w-4' />
            </div>

            <div className='min-w-0 flex-1'>
              <div className='flex items-center gap-2 text-sm font-medium'>
                Install from URL
                <span className='rounded-full border px-2 py-0.5 text-[10px] uppercase tracking-wide text-muted-foreground'>
                  Recommended
                </span>
              </div>
              <p className='mt-1 text-sm text-muted-foreground'>
                Best for published apps and updates.
              </p>
            </div>
          </div>

          <div className='flex flex-col gap-2'>
            <div className='relative w-full'>
              <LinkIcon className='pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground' />
              <Input
                value={urlInput}
                onChange={(e) => onUrlInputChange(e.target.value)}
                placeholder='https://example.com/my-app/'
                disabled={installing}
                className='pl-9'
                onKeyDown={(e) => {
                  if (
                    e.key === 'Enter' &&
                    !installing &&
                    urlInput.trim().length > 0
                  ) {
                    e.preventDefault();
                    void onPreviewUrl();
                  }
                }}
              />
            </div>

            <div className='flex justify-end'>
              <Button
                onClick={() => void onPreviewUrl()}
                disabled={installing || urlInput.trim().length === 0}
                className='min-w-[140px]'
              >
                Preview URL
              </Button>
            </div>
          </div>
        </div>

        <div className='rounded-xl border border-dashed p-4'>
          <div className='mb-3 flex items-start gap-3'>
            <div className='mt-0.5 rounded-lg border bg-background p-2 text-muted-foreground'>
              <Package className='h-4 w-4' />
            </div>

            <div className='min-w-0 flex-1'>
              <div className='text-sm font-medium'>Install from ZIP</div>
              <p className='mt-1 text-sm text-muted-foreground'>
                Useful for local builds, testing, or manual package installs.
              </p>
            </div>
          </div>

          <Button
            variant='outline'
            onClick={handleSelectZip}
            disabled={installing}
            className='w-full justify-center gap-2 md:w-auto'
          >
            <Upload className='h-4 w-4' />
            Select .zip package
          </Button>
        </div>

        {error ? (
          <div className='rounded-lg border border-destructive/30 bg-destructive/5 px-3 py-2 text-sm text-destructive'>
            {error}
          </div>
        ) : null}
      </CardContent>
    </Card>
  );
}
