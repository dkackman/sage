import { InstallAppForm } from '@/components/apps/InstallAppForm';
import { InstalledAppCard } from '@/components/apps/InstalledAppCard';
import { useApps } from '@/hooks/useApps';
import { CorruptedAppCard } from '@/components/apps/CorruptedAppCard.tsx';
import { SageGrantedPermissions } from '@/bindings.ts';
import { useAppRuntimes } from '@/hooks/useAppRuntimes.ts';
import { Button } from '@/components/ui/button.tsx';
import { Link } from 'react-router-dom';

export function Apps() {
  const {
    apps,
    loading,
    error,
    previewAppZip,
    previewAppUrl,
    installApp,
    installAppUrl,
    uninstallApp,
  } = useApps();
  const runtimes = useAppRuntimes();

  return (
    <div className='flex-1 overflow-auto'>
      <div className='mx-auto w-full max-w-6xl p-4 md:p-6 space-y-8'>
        <div className='space-y-2'>
          <h1 className='text-2xl font-semibold tracking-tight'>Apps</h1>
          <p className='text-muted-foreground'>
            Install and manage Sage app packages.
          </p>
          <Button asChild variant='outline'>
            <Link to='/apps/task-manager'>
              Task Manager ({runtimes.length})
            </Link>
          </Button>
        </div>

        <InstallAppForm
          onPreviewZip={previewAppZip}
          onPreviewUrl={previewAppUrl}
          onInstallZip={installApp}
          onInstallUrl={installAppUrl}
        />

        <section className='space-y-4'>
          <h2 className='text-lg font-semibold'>Installed</h2>

          {loading ? (
            <div className='rounded-lg border p-6 text-sm text-muted-foreground'>
              Loading apps...
            </div>
          ) : error ? (
            <div className='rounded-lg border border-destructive/30 p-6 text-sm text-destructive'>
              {error}
            </div>
          ) : apps.length > 0 ? (
            <div className='grid gap-4'>
              {apps.map((app) =>
                app.kind === 'installed' ? (
                  <InstalledAppCard
                    key={app.id}
                    app={app}
                    onUninstall={() => uninstallApp(app.id)}
                  />
                ) : (
                  <CorruptedAppCard
                    key={app.id}
                    app={app}
                    onRemove={() => uninstallApp(app.id)}
                  />
                ),
              )}
            </div>
          ) : (
            <div className='rounded-lg border p-6 text-sm text-muted-foreground'>
              No apps installed.
            </div>
          )}
        </section>
      </div>
    </div>
  );
}

