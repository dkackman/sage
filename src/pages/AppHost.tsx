import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { useApps } from '@/hooks/useApps';
import { handleBridgeRequest, isBridgeRequest } from '@/lib/apps/bridge';
import { ArrowLeft } from 'lucide-react';
import { useEffect, useMemo, useRef } from 'react';
import { Link, useParams } from 'react-router-dom';

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
  const iframeRef = useRef<HTMLIFrameElement | null>(null);

  const app = getApp(appId);

  const entrySrc = useMemo(() => {
    if (!app) {
      return null;
    }

    return `sage-app://${app.id}/index.html`;
  }, [app]);

  useEffect(() => {
    if (!app) {
      return;
    }

    function onMessage(event: MessageEvent) {
      const iframeWindow = iframeRef.current?.contentWindow;
      if (!iframeWindow) {
        return;
      }

      if (event.source !== iframeWindow) {
        return;
      }

      if (!isBridgeRequest(event.data)) {
        return;
      }

      void handleBridgeRequest({ app }, event.data).then((response) => {
        iframeWindow.postMessage(response, '*');
      });
    }

    window.addEventListener('message', onMessage);
    return () => {
      window.removeEventListener('message', onMessage);
    };
  }, [app]);

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
    <div className='flex-1 overflow-auto'>
      <div className='mx-auto w-full max-w-7xl p-4 md:p-6 space-y-4 h-full'>
        <div className='flex items-center justify-between gap-4'>
          <Button asChild variant='ghost' className='pl-0'>
            <Link to='/apps'>
              <ArrowLeft className='h-4 w-4 mr-2' />
              Back to Apps
            </Link>
          </Button>
        </div>

        <div className='space-y-1'>
          <h1 className='text-2xl font-semibold tracking-tight'>{app.name}</h1>
          <p className='text-xs text-muted-foreground break-all'>
            App URL: {entrySrc}
          </p>
        </div>

        {!entrySrc ? (
          <Alert>
            <AlertTitle>Invalid app entry</AlertTitle>
            <AlertDescription>
              Sage could not resolve the installed app entry.
            </AlertDescription>
          </Alert>
        ) : (
          <div className='rounded-xl border overflow-hidden bg-background h-[calc(100vh-220px)] min-h-[500px]'>
            <iframe
              ref={iframeRef}
              key={app.id}
              src={entrySrc}
              title={app.name}
              className='w-full h-full border-0'
              sandbox='allow-scripts allow-same-origin allow-forms allow-popups allow-downloads'
            />
          </div>
        )}
      </div>
    </div>
  );
}

