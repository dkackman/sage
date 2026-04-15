import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { useApps } from '@/hooks/useApps';
import {
  handleBridgeRequest,
  isBridgeRequest,
  type SageBridgeRequest,
} from '@/lib/apps/bridge';
import { openAppWindow } from '@/lib/apps/openAppWindow';
import { LogicalPosition, LogicalSize } from '@tauri-apps/api/dpi';
import { getCurrentWebview, Webview } from '@tauri-apps/api/webview';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { ArrowLeft } from 'lucide-react';
import { useEffect, useMemo, useRef } from 'react';
import { Link, useParams } from 'react-router-dom';

interface SageBridgeEventPayload {
  sourceLabel: string;
  appId: string;
  request: SageBridgeRequest;
}

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

  const app = getApp(appId);

  const entrySrc = useMemo(() => {
    if (!app) {
      return null;
    }

    return `sage-app://${app.id}/index.html`;
  }, [app]);

  useEffect(() => {
    if (!app || !entrySrc || !containerRef.current) {
      return;
    }

    const installedApp = app;
    const inlineLabel = `app-inline-${installedApp.id}`;
    const hostWindow = getCurrentWindow();
    const hostWebview = getCurrentWebview();

    let disposed = false;
    let embeddedWebview: Webview | null = null;
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
      const currentWebview = embeddedWebview;
      const container = containerRef.current;

      if (disposed || !currentWebview || !container) {
        return;
      }

      const rect = container.getBoundingClientRect();

      const width = Math.max(1, Math.round(rect.width));
      const height = Math.max(1, Math.round(rect.height));
      const x = Math.round(rect.left);
      const y = Math.round(rect.top);

      await currentWebview.setPosition(new LogicalPosition(x, y));
      await currentWebview.setSize(new LogicalSize(width, height));
    };

    const scheduleSyncBounds = () => {
      requestAnimationFrame(() => {
        void syncBounds().catch((err) => {
          console.error('Failed to sync embedded app webview bounds:', err);
        });
      });
    };

    const mount = async () => {
      const existing = await Webview.getByLabel(inlineLabel);

      if (disposed) {
        return;
      }

      if (existing) {
        embeddedWebview = existing;
      } else {
        embeddedWebview = new Webview(hostWindow, inlineLabel, {
          url: entrySrc,
          x: 0,
          y: 0,
          width: 1,
          height: 1,
          focus: true,
        });

        const createdWebview = embeddedWebview;

        await new Promise<void>((resolve, reject) => {
          const createdPromise = createdWebview.once('tauri://created', () => {
            resolve();
          });

          const errorPromise = createdWebview.once('tauri://error', (event) => {
            const payload =
              typeof event.payload === 'string'
                ? event.payload
                : JSON.stringify(event.payload);
            reject(new Error(payload));
          });

          void Promise.all([createdPromise, errorPromise]);
        });
      }

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

      unlistenBridge = await hostWebview.listen<SageBridgeEventPayload>(
        'sage-bridge:request',
        ({ payload }) => {
          if (
            !payload ||
            payload.appId !== installedApp.id ||
            !isBridgeRequest(payload.request)
          ) {
            return;
          }

          void handleBridgeRequest({ app: installedApp }, payload.request).then(
            (response) => {
              void hostWebview.emitTo(
                payload.sourceLabel,
                'sage-bridge:response',
                response,
              );
            },
          );
        },
      );
    };

    void mount().catch((err) => {
      console.error('Failed to mount embedded app webview:', err);
    });

    return () => {
      disposed = true;

      resizeObserver?.disconnect();
      unlistenBridge?.();
      removeWindowResize?.();
      clearDelayedSyncTimers();

      if (embeddedWebview) {
        void embeddedWebview.close().catch((err) => {
          console.error('Failed to close embedded app webview:', err);
        });
      }
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

          <Button
            variant='outline'
            onClick={() => {
              void openAppWindow(app.id, app.name).catch((err) => {
                console.error('Failed to open app window:', err);
              });
            }}
          >
            Open in window
          </Button>
        </div>

        <div className='space-y-1 shrink-0'>
          <h1 className='text-2xl font-semibold tracking-tight'>{app.name}</h1>
          <p className='text-xs break-all text-muted-foreground'>
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

