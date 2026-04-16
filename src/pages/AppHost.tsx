import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { useRef } from 'react';
import { useParams } from 'react-router-dom';
import { useApps } from '@/contexts/AppsContext';
import { useAppEmbeddedRuntime } from '@/hooks/useAppEmbeddedRuntime.ts';
import { useAppsWorkspaceOutletContext } from '@/pages/AppsWorkspace.tsx';

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
  const containerRef = useRef<HTMLDivElement | null>(null);
  const { getApp, loading } = useApps();
  const { requestApproval } = useAppsWorkspaceOutletContext();

  const app = getApp(appId);

  useAppEmbeddedRuntime({
    app,
    containerRef,
    requestApproval,
  });

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
    <div className='flex h-full min-h-0 w-full flex-col overflow-hidden'>
      <div className='flex-1 min-h-0'>
        <div
          ref={containerRef}
          className='h-full w-full overflow-hidden bg-background'
        />
      </div>
    </div>
  );
}
