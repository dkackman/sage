import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { InstallAppForm } from '@/components/apps/InstallAppForm';
import { CorruptedAppCard } from '@/components/apps/CorruptedAppCard';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { SageAppPackageManifest, SageAppUrlPreview } from '@/bindings.ts';
import { invoke } from '@tauri-apps/api/core';
import { useApps } from '@/contexts/AppsContext.tsx';
import { LayoutGrid, Plus } from 'lucide-react';
import { useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';

type InstalledEntry = ReturnType<typeof useApps>['apps'][number] & {
  kind: 'installed';
};

type AppContextMenuState = {
  app: InstalledEntry;
  x: number;
  y: number;
} | null;

function clampContextMenuPosition(args: {
  x: number;
  y: number;
  containerWidth: number;
  containerHeight: number;
}) {
  const menuWidth = 220;
  const menuHeight = 220;
  const padding = 8;

  return {
    x: Math.max(
      padding,
      Math.min(args.x, args.containerWidth - menuWidth - padding),
    ),
    y: Math.max(
      padding,
      Math.min(args.y, args.containerHeight - menuHeight - padding),
    ),
  };
}

export function Apps() {
  const navigate = useNavigate();
  const [installOpen, setInstallOpen] = useState(false);
  const [contextMenu, setContextMenu] = useState<AppContextMenuState>(null);
  const pageRef = useRef<HTMLDivElement | null>(null);

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

  const installedApps = useMemo(
    () =>
      apps.filter(
        (entry): entry is InstalledEntry => entry.kind === 'installed',
      ),
    [apps],
  );

  const corruptedApps = useMemo(
    () => apps.filter((entry) => entry.kind === 'corrupted'),
    [apps],
  );

  useEffect(() => {
    if (!contextMenu) {
      return;
    }

    const handleClose = () => {
      setContextMenu(null);
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setContextMenu(null);
      }
    };

    window.addEventListener('click', handleClose);
    window.addEventListener('resize', handleClose);
    window.addEventListener('scroll', handleClose, true);
    window.addEventListener('keydown', handleKeyDown);

    return () => {
      window.removeEventListener('click', handleClose);
      window.removeEventListener('resize', handleClose);
      window.removeEventListener('scroll', handleClose, true);
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, [contextMenu]);

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

  const contextMenuPreview = contextMenu
    ? updateAvailability[contextMenu.app.id]
    : null;
  const contextMenuBusy = contextMenu
    ? (busyAppIds[contextMenu.app.id] ?? false)
    : false;

  return (
    <>
      <div
        ref={pageRef}
        className='relative flex h-full min-h-0 flex-col overflow-hidden'
      >
        <div className='mx-auto flex w-full max-w-7xl shrink-0 items-center justify-between gap-4 p-4 md:p-6'>
          <div>
            <h1 className='text-2xl font-semibold tracking-tight'>Apps</h1>
            <p className='text-sm text-muted-foreground'>
              Launch and manage installed Sage apps.
            </p>
          </div>

          <div className='flex items-center gap-2'>
            <Button
              variant='outline'
              onClick={() => {
                setInstallOpen(true);
              }}
            >
              <Plus className='mr-2 h-4 w-4' />
              Install App
            </Button>

            <Button
              variant='outline'
              onClick={() => {
                navigate('/apps/task-manager');
              }}
            >
              <LayoutGrid className='mr-2 h-4 w-4' />
              Task Manager
            </Button>
          </div>
        </div>

        <div className='mx-auto w-full max-w-7xl flex-1 min-h-0 overflow-auto px-4 pb-4 md:px-6 md:pb-6'>
          {error ? (
            <Alert className='mb-6'>
              <AlertTitle>Apps error</AlertTitle>
              <AlertDescription>{error}</AlertDescription>
            </Alert>
          ) : null}

          {installedApps.length === 0 ? (
            <Alert className='mb-6'>
              <AlertTitle>No apps installed</AlertTitle>
              <AlertDescription>
                Install a Sage app package to get started.
              </AlertDescription>
            </Alert>
          ) : null}

          {installedApps.length > 0 ? (
            <div className='grid grid-cols-2 gap-4 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6'>
              {installedApps.map((app) => {
                const iconSrc = `sage-app://${app.id}/${app.iconFile}`;

                return (
                  <button
                    key={app.id}
                    type='button'
                    onClick={() => {
                      navigate(`/apps/${app.id}`);
                    }}
                    onContextMenu={(event) => {
                      event.preventDefault();

                      const pageEl = pageRef.current;
                      if (!pageEl) {
                        return;
                      }

                      const pageRect = pageEl.getBoundingClientRect();

                      const localX = event.clientX - pageRect.left;
                      const localY = event.clientY - pageRect.top;

                      const position = clampContextMenuPosition({
                        x: localX,
                        y: localY,
                        containerWidth: pageRect.width,
                        containerHeight: pageRect.height,
                      });

                      setContextMenu({
                        app,
                        x: position.x,
                        y: position.y,
                      });
                    }}
                    className='group flex flex-col items-center gap-3 rounded-2xl p-4 text-center transition-colors hover:bg-muted/50'
                  >
                    <div className='flex h-20 w-20 items-center justify-center overflow-hidden rounded-2xl border bg-background shadow-sm'>
                      <img
                        src={iconSrc}
                        alt=''
                        className='h-full w-full object-cover'
                      />
                    </div>

                    <div className='min-w-0 w-full'>
                      <div className='truncate text-sm font-medium'>
                        {app.name}
                      </div>
                      <div className='truncate text-xs text-muted-foreground'>
                        v{app.version}
                      </div>
                    </div>
                  </button>
                );
              })}
            </div>
          ) : null}

          {corruptedApps.length > 0 ? (
            <div className='mt-8 space-y-4'>
              <div>
                <h2 className='text-lg font-semibold tracking-tight'>
                  Corrupted apps
                </h2>
                <p className='text-sm text-muted-foreground'>
                  These app installations could not be loaded correctly.
                </p>
              </div>

              <div className='space-y-4'>
                {corruptedApps.map((entry) => (
                  <CorruptedAppCard
                    key={entry.id}
                    app={entry}
                    onRemove={() => uninstallApp(entry.id)}
                  />
                ))}
              </div>
            </div>
          ) : null}
        </div>
      </div>

      <Dialog
        open={installOpen}
        onOpenChange={(open) => {
          setInstallOpen(open);
        }}
      >
        <DialogContent className='max-w-2xl'>
          <DialogHeader>
            <DialogTitle>Install App</DialogTitle>
          </DialogHeader>

          <InstallAppForm
            onPreviewZip={(zipPath: string) =>
              invoke<SageAppPackageManifest>('preview_app_zip', { zipPath })
            }
            onPreviewUrl={(appUrl: string) =>
              invoke<SageAppUrlPreview>('preview_app_url', { appUrl })
            }
            onInstallZip={async (zipPath, permissions) => {
              await installApp(zipPath, permissions);
              setInstallOpen(false);
            }}
            onInstallUrl={async (appUrl, permissions) => {
              await invoke('install_app_url', {
                appUrl,
                grantedPermissions: permissions,
              });
              await refresh();
              setInstallOpen(false);
            }}
          />
        </DialogContent>
      </Dialog>

      {contextMenu ? (
        <>
          <div
            className='absolute inset-0 z-40'
            onClick={() => {
              setContextMenu(null);
            }}
          />

          <div
            className='absolute z-50 w-[220px] rounded-xl border bg-popover p-1 shadow-lg'
            style={{
              left: `${contextMenu.x}px`,
              top: `${contextMenu.y}px`,
            }}
            onClick={(event) => {
              event.stopPropagation();
            }}
          >
            <button
              type='button'
              className='flex w-full rounded-lg px-3 py-2 text-left text-sm hover:bg-muted'
              onClick={() => {
                navigate(`/apps/${contextMenu.app.id}`);
                setContextMenu(null);
              }}
            >
              Open
            </button>

            <button
              type='button'
              className='flex w-full rounded-lg px-3 py-2 text-left text-sm hover:bg-muted disabled:opacity-50'
              disabled={contextMenuBusy}
              onClick={() => {
                void checkForUpdate(contextMenu.app.id).finally(() => {
                  setContextMenu(null);
                });
              }}
            >
              Check for update
            </button>

            {contextMenuPreview ? (
              <>
                <button
                  type='button'
                  className='flex w-full rounded-lg px-3 py-2 text-left text-sm hover:bg-muted disabled:opacity-50'
                  disabled={contextMenuBusy}
                  onClick={() => {
                    void downloadUpdate(contextMenu.app.id).finally(() => {
                      setContextMenu(null);
                    });
                  }}
                >
                  Download update
                </button>

                <button
                  type='button'
                  className='flex w-full rounded-lg px-3 py-2 text-left text-sm hover:bg-muted disabled:opacity-50'
                  disabled={contextMenuBusy}
                  onClick={() => {
                    void performAppUpdate(contextMenu.app.id, {
                      restartIfRunning: true,
                      visibleAfterRestart: false,
                    }).finally(() => {
                      setContextMenu(null);
                    });
                  }}
                >
                  Apply update
                </button>
              </>
            ) : null}

            <div className='my-1 h-px bg-border' />

            <button
              type='button'
              className='flex w-full rounded-lg px-3 py-2 text-left text-sm text-muted-foreground opacity-60'
              disabled
            >
              Change permissions
            </button>

            <button
              type='button'
              className='flex w-full rounded-lg px-3 py-2 text-left text-sm text-destructive hover:bg-muted'
              onClick={() => {
                void uninstallApp(contextMenu.app.id).finally(() => {
                  setContextMenu(null);
                });
              }}
            >
              Uninstall
            </button>
          </div>
        </>
      ) : null}
    </>
  );
}
