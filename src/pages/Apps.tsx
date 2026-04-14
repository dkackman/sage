import { InstallAppForm } from '@/components/apps/InstallAppForm';
import { InstalledAppCard } from '@/components/apps/InstalledAppCard';
import { useApps } from '@/hooks/useApps';

export function Apps() {
  const { apps, loading, error, installApp, uninstallApp } = useApps();

  return (
    <div className='flex-1 overflow-auto'>
      <div className='mx-auto w-full max-w-6xl p-4 md:p-6 space-y-8'>
        <div className='space-y-2'>
          <h1 className='text-2xl font-semibold tracking-tight'>Apps</h1>
          <p className='text-muted-foreground'>
            Install and manage Sage app packages.
          </p>
        </div>

        <InstallAppForm onInstall={installApp} />

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
              {apps.map((app) => (
                <InstalledAppCard
                  key={app.id}
                  app={app}
                  onUninstall={() => uninstallApp(app.id)}
                />
              ))}
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

