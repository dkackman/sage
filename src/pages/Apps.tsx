import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { InstallAppForm } from '@/components/apps/InstallAppForm';
import { CorruptedAppCard } from '@/components/apps/CorruptedAppCard';
import { AppsLaunchpadContextMenu } from '@/components/apps/AppsLaunchpadContextMenu';
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
import { useAppRuntimes } from '@/hooks/useAppRuntimes.ts';
import { clearAppRuntimeBrowsingData } from '@/lib/apps/runtimeRegistry';
import { formatCapabilityLabel } from '@/lib/apps/sandbox';
import { LayoutGrid, Plus } from 'lucide-react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { PermissionsEditor } from '@/components/apps/permissions/PermissionsEditor.tsx';
import { getSandboxLaunchDecision } from '@/lib/apps/sandboxPolicy.ts';
import { useSandbox } from '@/contexts/SandboxContext.tsx';

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
  const menuWidth = 260;
  const menuHeight = 260;
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
  const runtimes = useAppRuntimes();
  const [updateCheckStateByAppId, setUpdateCheckStateByAppId] = useState<
    Record<string, 'idle' | 'checking' | 'up_to_date'>
  >({});
  const [clearingDataByAppId, setClearingDataByAppId] = useState<
    Record<string, boolean>
  >({});
  const [clearDataErrorByAppId, setClearDataErrorByAppId] = useState<
    Record<string, string | null>
  >({});
  const [permissionsDialogApp, setPermissionsDialogApp] =
    useState<InstalledEntry | null>(null);

  const {
    apps,
    loading,
    error,
    refresh,
    installApp,
    installUrlApp,
    uninstallApp,
    checkForUpdate,
    performAppUpdate,
    updateAvailability,
    busyAppIds,
  } = useApps();

  const runningAppIds = useMemo(() => {
    return new Set(runtimes.map((runtime) => runtime.appId));
  }, [runtimes]);

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
  const { sandboxState, rerunSandboxTests } = useSandbox();

  const contextMenuPreview = contextMenu
    ? updateAvailability[contextMenu.app.id]
    : null;
  const contextMenuBusy = contextMenu
    ? (busyAppIds[contextMenu.app.id] ?? false)
    : false;
  const contextMenuCheckState = contextMenu
    ? (updateCheckStateByAppId[contextMenu.app.id] ?? 'idle')
    : 'idle';
  const contextMenuAppIsRunning = contextMenu
    ? runningAppIds.has(contextMenu.app.id)
    : false;
  const contextMenuClearDataBusy = contextMenu
    ? (clearingDataByAppId[contextMenu.app.id] ?? false)
    : false;
  const contextMenuClearDataError = contextMenu
    ? (clearDataErrorByAppId[contextMenu.app.id] ?? null)
    : null;

  const closeContextMenu = useCallback(() => {
    setContextMenu((prevContextMenu) => {
      if (prevContextMenu) {
        setUpdateCheckStateByAppId((prev) => {
          if (prev[prevContextMenu.app.id] !== 'up_to_date') {
            return prev;
          }

          return {
            ...prev,
            [prevContextMenu.app.id]: 'idle',
          };
        });
      }

      return null;
    });
  }, []);

  async function handleCheckForUpdate(appId: string) {
    setUpdateCheckStateByAppId((prev) => ({
      ...prev,
      [appId]: 'checking',
    }));

    try {
      await checkForUpdate(appId);
    } finally {
      setUpdateCheckStateByAppId((prev) => ({
        ...prev,
        [appId]: 'up_to_date',
      }));
    }
  }

  function openPermissionsDialog(app: InstalledEntry) {
    setPermissionsDialogApp(app);
  }

  function closePermissionsDialog() {
    setPermissionsDialogApp(null);
  }

  const handleClearData = useCallback(
    async (app: InstalledEntry, reopen: boolean) => {
      setClearingDataByAppId((prev) => ({
        ...prev,
        [app.id]: true,
      }));
      setClearDataErrorByAppId((prev) => ({
        ...prev,
        [app.id]: null,
      }));

      try {
        await clearAppRuntimeBrowsingData(app);

        if (reopen) {
          closeContextMenu();

          const { restartAppRuntime } =
            await import('@/lib/apps/restartAppRuntime');

          await restartAppRuntime(app, {
            visible: true,
          });

          navigate(`/apps/${app.id}`);
        }

        await refresh();
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);

        setClearDataErrorByAppId((prev) => ({
          ...prev,
          [app.id]: message,
        }));
      } finally {
        setClearingDataByAppId((prev) =>
          Object.fromEntries(
            Object.entries(prev).filter(([key]) => key !== app.id),
          ),
        );
      }
    },
    [navigate, refresh, closeContextMenu],
  );

  const handleApplyPermissions = useCallback(
    async (app: InstalledEntry, nextPermissions: string[]) => {
      const prev = new Set(app.grantedPermissions);
      const next = new Set(nextPermissions);

      const storageChanged =
        prev.has('persistent_storage') !== next.has('persistent_storage');

      await invoke('apps_update_permissions', {
        appId: app.id,
        grantedPermissions: nextPermissions,
      });

      if (storageChanged) {
        await clearAppRuntimeBrowsingData(app);
      }

      const isRunning = runningAppIds.has(app.id);
      if (isRunning) {
        const { restartAppRuntime } =
          await import('@/lib/apps/restartAppRuntime');

        await restartAppRuntime(app, { visible: true });
        navigate(`/apps/${app.id}`);
      }

      await refresh();
      closePermissionsDialog();
    },
    [runningAppIds, navigate, refresh],
  );

  useEffect(() => {
    if (!contextMenu || contextMenuCheckState !== 'up_to_date') {
      return;
    }

    const timeoutId = window.setTimeout(() => {
      setUpdateCheckStateByAppId((prev) => {
        if (prev[contextMenu.app.id] !== 'up_to_date') {
          return prev;
        }

        return {
          ...prev,
          [contextMenu.app.id]: 'idle',
        };
      });
    }, 3000);

    return () => {
      window.clearTimeout(timeoutId);
    };
  }, [contextMenu, contextMenuCheckState]);

  useEffect(() => {
    if (!contextMenu) {
      return;
    }

    const handleClose = () => {
      if (clearingDataByAppId[contextMenu.app.id]) {
        return;
      }

      closeContextMenu();
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        if (clearingDataByAppId[contextMenu.app.id]) {
          return;
        }

        closeContextMenu();
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
  }, [contextMenu, clearingDataByAppId, closeContextMenu]);

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
            <Button
              variant='outline'
              disabled={sandboxState.overallCriticalStatus === 'running'}
              onClick={() => {
                void rerunSandboxTests();
              }}
            >
              {sandboxState.overallCriticalStatus === 'running'
                ? 'Running sandbox tests...'
                : 'Re-run sandbox tests'}
            </Button>
          </div>
        </div>

        <div className='mx-auto w-full max-w-7xl flex-1 min-h-0 overflow-auto px-4 pb-4 md:px-6 md:pb-6'>
          <Alert className='mb-6'>
            <AlertTitle>
              {sandboxState.overallCriticalStatus === 'running'
                ? 'Sandbox tests are running'
                : sandboxState.overallCriticalStatus === 'passed'
                  ? 'Sandbox tests passed'
                  : sandboxState.overallCriticalStatus === 'failed'
                    ? 'Sandbox tests failed'
                    : 'Sandbox tests are pending'}
            </AlertTitle>

            <AlertDescription className='space-y-3'>
              <div>
                Apps are allowed to launch only when all required sandbox
                capabilities have passed.
              </div>

              <div className='space-y-1 text-xs text-muted-foreground'>
                {Object.entries(sandboxState.capabilities).map(
                  ([capability, result]) => (
                    <div key={capability}>
                      {formatCapabilityLabel(capability as never)} —{' '}
                      {result.status}
                      {result.details ? ` — ${result.details}` : ''}
                    </div>
                  ),
                )}
              </div>

              <div>
                <Button
                  variant='outline'
                  onClick={() => {
                    void rerunSandboxTests();
                  }}
                >
                  Re-run tests
                </Button>
              </div>
            </AlertDescription>
          </Alert>

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
                const launchGate = getSandboxLaunchDecision({
                  app,
                  sandboxState,
                });

                return (
                  <button
                    key={app.id}
                    type='button'
                    onClick={() => {
                      const decision = getSandboxLaunchDecision({
                        app,
                        sandboxState,
                      });

                      if (!decision.allowed) {
                        return;
                      }

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

                      setClearDataErrorByAppId((prev) => ({
                        ...prev,
                        [app.id]: null,
                      }));

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

                      {!launchGate.allowed ? (
                        <div className='mt-1 text-xs text-amber-600'>
                          {launchGate.title}
                        </div>
                      ) : null}
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

        <AppsLaunchpadContextMenu
          open={!!contextMenu}
          x={contextMenu?.x ?? 0}
          y={contextMenu?.y ?? 0}
          busy={contextMenuBusy}
          hasUpdate={!!contextMenuPreview}
          isRunning={contextMenuAppIsRunning}
          updateCheckState={contextMenuCheckState}
          clearDataBusy={contextMenuClearDataBusy}
          clearDataError={contextMenuClearDataError}
          onClose={closeContextMenu}
          onOpen={() => {
            if (!contextMenu) {
              return;
            }

            setUpdateCheckStateByAppId((prev) => ({
              ...prev,
              [contextMenu.app.id]: 'idle',
            }));
            navigate(`/apps/${contextMenu.app.id}`);
            closeContextMenu();
          }}
          onCheckForUpdate={() => {
            if (!contextMenu) {
              return;
            }

            void handleCheckForUpdate(contextMenu.app.id);
          }}
          onUpdate={() => {
            if (!contextMenu) {
              return;
            }

            void performAppUpdate(
              contextMenu.app.id,
              contextMenu.app.grantedPermissions,
              {
                restartIfRunning: true,
                visibleAfterRestart: contextMenuAppIsRunning,
              },
            );
          }}
          onChangePermissions={() => {
            if (!contextMenu) {
              return;
            }

            openPermissionsDialog(contextMenu.app);
          }}
          onClearData={() => {
            if (!contextMenu) {
              return;
            }

            const targetApp = contextMenu.app;
            const shouldReopen = runningAppIds.has(targetApp.id);

            void handleClearData(targetApp, shouldReopen);
          }}
          onUninstall={() => {
            if (!contextMenu) {
              return;
            }

            setUpdateCheckStateByAppId((prev) => ({
              ...prev,
              [contextMenu.app.id]: 'idle',
            }));

            void uninstallApp(contextMenu.app.id).finally(() => {
              closeContextMenu();
            });
          }}
        />
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
              await installUrlApp(appUrl, permissions);
              setInstallOpen(false);
            }}
          />
        </DialogContent>
      </Dialog>

      <Dialog
        open={!!permissionsDialogApp}
        onOpenChange={(open) => {
          if (!open) closePermissionsDialog();
        }}
      >
        <DialogContent className='max-w-md'>
          <DialogHeader>
            <DialogTitle>Change permissions</DialogTitle>
          </DialogHeader>

          {permissionsDialogApp ? (
            <PermissionsEditor
              app={permissionsDialogApp}
              onCancel={closePermissionsDialog}
              onApply={(next) =>
                handleApplyPermissions(permissionsDialogApp, next)
              }
            />
          ) : null}
        </DialogContent>
      </Dialog>
    </>
  );
}
