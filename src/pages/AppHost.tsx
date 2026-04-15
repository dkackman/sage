import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { useApps } from '@/hooks/useApps';
import {
  handleBridgeRequest,
  isBridgeRequest,
  type SageBridgeEventPayload,
} from '@/lib/apps/bridge';
import {
  ensureInlineRuntime,
  getRuntimeWebview,
  killRuntime,
  markRuntimeVisible,
} from '@/lib/apps/runtimeRegistry';
import { LogicalPosition, LogicalSize } from '@tauri-apps/api/dpi';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { ArrowLeft } from 'lucide-react';
import { useEffect, useMemo, useRef, useState } from 'react';
import { Link, useNavigate, useParams } from 'react-router-dom';
import { useAppRuntimePresence } from '@/hooks/useAppRuntimePresence.ts';
import { invoke } from '@tauri-apps/api/core';
import { SageAppUrlPreview } from '@/bindings.ts';

function AppNotFound() {
  return (
    <div className='mx-auto w-full max-w-4xl p-4 md:p-6'>
      <Alert>
        <AlertTitle>App not found</AlertTitle>
        <AlertDescription>This app is not installed.</AlertDescription>
      </Alert>
    </div>
  );
}

export function AppHost() {
  const { appId = '' } = useParams();
  const { getApp, loading } = useApps();
  const containerRef = useRef<HTMLDivElement | null>(null);
  const navigate = useNavigate();

  const app = getApp(appId);
  const isRunning = useAppRuntimePresence(appId);
  const [updatePreview, setUpdatePreview] = useState<null | {
    manifest: { version: string };
  }>(null);
  const [checkingUpdate, setCheckingUpdate] = useState(false);
  const [applyingUpdate, setApplyingUpdate] = useState(false);

  const handleUpdateAndReopen = async () => {
    if (!app) {
      return;
    }

    try {
      setApplyingUpdate(true);

      const refreshedPreview = await invoke<SageAppUrlPreview | null>(
        'check_app_update',
        {
          appId: app.id,
        },
      );

      if (refreshedPreview) {
        await invoke('download_app_update', { appId: app.id });
      }

      await invoke('apply_app_update', {
        appId: app.id,
        grantedPermissions: app.grantedPermissions,
      });

      window.location.reload();
    } finally {
      setApplyingUpdate(false);
    }
  };

  const entrySrc = useMemo(() => {
    if (!app) {
      return null;
    }

    return app.source.kind === 'url'
      ? app.source.appUrl
      : `sage-app://${app.id}/index.html`;
  }, [app]);

  useEffect(() => {
    if (!app || app.source?.kind !== 'url') {
      setUpdatePreview(null);
      return;
    }

    let cancelled = false;

    const check = async () => {
      try {
        setCheckingUpdate(true);
        const preview = await invoke<SageAppUrlPreview | null>(
          'check_app_update',
          {
            appId: app.id,
          },
        );

        if (!cancelled) {
          setUpdatePreview(preview);
        }
      } catch {
        if (!cancelled) {
          setUpdatePreview(null);
        }
      } finally {
        if (!cancelled) {
          setCheckingUpdate(false);
        }
      }
    };

    void check();

    const intervalId = window.setInterval(
      () => {
        void check();
      },
      10 * 60 * 1000,
    );

    return () => {
      cancelled = true;
      window.clearInterval(intervalId);
    };
  }, [app]);
  useEffect(() => {
    if (!app || !entrySrc || !containerRef.current) {
      return;
    }

    const installedApp = app;
    const hostWebview = getCurrentWebview();

    let disposed = false;
    let resizeObserver: ResizeObserver | null = null;
    let unlistenBridge: (() => void) | null = null;
    let removeWindowResize: (() => void) | null = null;
    let delayedSyncTimers: number[] = [];

    const clearDelayedSyncTimers = () => {
      delayedSyncTimers.forEach((id) => {
        window.clearTimeout(id);
      });
      delayedSyncTimers = [];
    };

    const syncBounds = async () => {
      const webview = await getRuntimeWebview(installedApp.id);
      const container = containerRef.current;

      if (disposed || !webview || !container) {
        return;
      }

      const rect = container.getBoundingClientRect();
      const width = Math.max(1, Math.round(rect.width));
      const height = Math.max(1, Math.round(rect.height));
      const x = Math.round(rect.left);
      const y = Math.round(rect.top);

      await webview.setPosition(new LogicalPosition(x, y));
      await webview.setSize(new LogicalSize(width, height));
    };

    const scheduleSyncBounds = () => {
      requestAnimationFrame(() => {
        void syncBounds().catch((err) => {
          console.error('Failed to sync embedded app webview bounds:', err);
        });
      });
    };

    const mount = async () => {
      await ensureInlineRuntime(installedApp);
      await markRuntimeVisible(installedApp.id, true);

      scheduleSyncBounds();

      delayedSyncTimers = [0, 50, 150, 300].map((delay) =>
        window.setTimeout(() => {
          scheduleSyncBounds();
        }, delay),
      );

      resizeObserver = new ResizeObserver(() => {
        scheduleSyncBounds();
      });

      const container = containerRef.current;
      if (!container) {
        return;
      }

      resizeObserver.observe(container);

      const handleWindowResize = () => {
        scheduleSyncBounds();
      };

      window.addEventListener('resize', handleWindowResize);
      removeWindowResize = () => {
        window.removeEventListener('resize', handleWindowResize);
      };
      const expectedSourceLabel = `app-inline-${installedApp.id}`;

      unlistenBridge = await hostWebview.listen<SageBridgeEventPayload>(
        'sage-bridge:request',
        ({ payload }) => {
          if (
            !payload ||
            payload.sourceLabel !== expectedSourceLabel ||
            !isBridgeRequest(payload.request)
          ) {
            return;
          }

          void handleBridgeRequest(
            {
              app: installedApp,
              sourceLabel: payload.sourceLabel,
              emitEvent: async (event) => {
                await hostWebview.emitTo(
                  payload.sourceLabel,
                  'sage-bridge:event',
                  event,
                );
              },
            },
            payload.request,
          ).then((response) => {
            void hostWebview.emitTo(
              payload.sourceLabel,
              'sage-bridge:response',
              response,
            );
          });
        },
      );
    };

    void mount().catch((err) => {
      console.error('Failed to attach app runtime:', err);
    });

    return () => {
      disposed = true;
      void markRuntimeVisible(installedApp.id, false);
      resizeObserver?.disconnect();
      unlistenBridge?.();
      removeWindowResize?.();
      clearDelayedSyncTimers();
    };
  }, [app, entrySrc]);

  if (loading) {
    return (
      <div className='mx-auto w-full max-w-4xl p-4 md:p-6'>
        <Alert>
          <AlertTitle>Loading app...</AlertTitle>
          <AlertDescription>Please wait.</AlertDescription>
        </Alert>
      </div>
    );
  }

  if (!app) {
    return <AppNotFound />;
  }

  return (
    <div className='flex h-full min-h-0 flex-col'>
      <div className='mx-auto flex h-full min-h-0 w-full max-w-7xl flex-col gap-4 p-4 md:p-6'>
        <div className='flex items-center justify-between gap-4'>
          <Button asChild variant='ghost' className='pl-0'>
            <Link to='/apps'>
              <ArrowLeft className='mr-2 h-4 w-4' />
              Back to Apps
            </Link>
          </Button>
        </div>
        <div className='flex items-center gap-2'>
          <Button
            variant='destructive'
            onClick={() => {
              void killRuntime(app.id).then(() => {
                navigate('/apps');
              });
            }}
          >
            Exit App
          </Button>
        </div>

        {app.source?.kind === 'url' && updatePreview ? (
          <Alert>
            <AlertTitle>New version available</AlertTitle>
            <AlertDescription className='flex items-center justify-between gap-4'>
              <span>
                Version {updatePreview.manifest.version} is available for this
                app.
              </span>

              <Button
                variant='outline'
                disabled={checkingUpdate || applyingUpdate || !isRunning}
                onClick={() => {
                  void handleUpdateAndReopen();
                }}
              >
                {applyingUpdate ? 'Updating...' : 'Update and reopen'}
              </Button>
            </AlertDescription>
          </Alert>
        ) : null}

        <div className='shrink-0 space-y-1'>
          <h1 className='text-2xl font-semibold tracking-tight'>{app.name}</h1>
          <p className='break-all text-xs text-muted-foreground'>
            App URL: {entrySrc}
          </p>
        </div>

        <div className='flex-1 min-h-0'>
          <div
            ref={containerRef}
            className='h-full w-full overflow-hidden rounded-xl bg-background'
          />
        </div>
      </div>
    </div>
  );
}
