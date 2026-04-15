import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { InstallAppForm } from '@/components/apps/InstallAppForm';
import { InstalledAppCard } from '@/components/apps/InstalledAppCard';
import { CorruptedAppCard } from '@/components/apps/CorruptedAppCard';
import { Button } from '@/components/ui/button';
import { Link } from 'react-router-dom';
import { SageAppPackageManifest, SageAppUrlPreview } from '@/bindings.ts';
import { invoke } from '@tauri-apps/api/core';
import { useApps } from '@/contexts/AppsContext.tsx';

export function Apps() {
  const {
    apps,
    loading,
    error,
    refresh,
    installApp,
    uninstallApp,
    checkForUpdate,
    downloadUpdate,
    performAppUpdate,
    updateAvailability,
    busyAppIds,
  } = useApps();

  if (loading) {
    return (
      <div className='mx-auto w-full max-w-6xl p-4 md:p-6'>
        <Alert>
          <AlertTitle>Loading apps...</AlertTitle>
          <AlertDescription>Please wait.</AlertDescription>
        </Alert>
      </div>
    );
  }

  return (
    <div className='mx-auto w-full max-w-6xl space-y-6 p-4 md:p-6'>
      <div className='flex items-center justify-between gap-4'>
        <div>
          <h1 className='text-2xl font-semibold tracking-tight'>Apps</h1>
          <p className='text-sm text-muted-foreground'>
            Install and manage Sage apps.
          </p>
        </div>

        <Button asChild variant='outline'>
          <Link to='/apps/task-manager'>Task Manager</Link>
        </Button>
      </div>

      <InstallAppForm
        onPreviewZip={(zipPath: string) =>
          invoke<SageAppPackageManifest>('preview_app_zip', { zipPath })
        }
        onPreviewUrl={(appUrl: string) =>
          invoke<SageAppUrlPreview>('preview_app_url', { appUrl })
        }
        onInstallZip={installApp}
        onInstallUrl={async (appUrl, permissions) => {
          await invoke('install_app_url', {
            appUrl,
            grantedPermissions: permissions,
          });
          await refresh();
        }}
      />

      {error ? (
        <Alert>
          <AlertTitle>Apps error</AlertTitle>
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      ) : null}

      <div className='space-y-4'>
        {apps.length === 0 ? (
          <Alert>
            <AlertTitle>No apps installed</AlertTitle>
            <AlertDescription>
              Install a Sage app package to get started.
            </AlertDescription>
          </Alert>
        ) : null}

        {apps.map((entry) => {
          if (entry.kind === 'installed') {
            return (
              <InstalledAppCard
                key={entry.id}
                app={entry}
                updatePreview={updateAvailability[entry.id]}
                busy={busyAppIds[entry.id]}
                onUninstall={() => uninstallApp(entry.id)}
                onCheckForUpdate={async () => {
                  await checkForUpdate(entry.id);
                }}
                onDownloadUpdate={async () => {
                  await downloadUpdate(entry.id);
                }}
                onApplyUpdate={async () => {
                  await performAppUpdate(entry.id, {
                    restartIfRunning: true,
                    visibleAfterRestart: false,
                  });
                }}
              />
            );
          }

          return (
            <CorruptedAppCard
              key={entry.id}
              app={entry}
              onRemove={() => uninstallApp(entry.id)}
            />
          );
        })}
      </div>
    </div>
  );
}
