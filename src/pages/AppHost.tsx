import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
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
  const { getApp, getAppLaunchGate, rerunSandboxTests, loading } = useApps();
  const { requestApproval } = useAppsWorkspaceOutletContext();

  const app = getApp(appId);
  const gate = app ? getAppLaunchGate(app.id) : null;

  useAppEmbeddedRuntime({
    app: gate?.allowed ? app : null,
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

  if (gate && !gate.allowed) {
    return (
      <div className='mx-auto w-full max-w-4xl p-4 md:p-6'>
        <Alert>
          <AlertTitle>
            {gate.kind === 'running'
              ? 'Sandbox tests are still running'
              : 'App launch is blocked'}
          </AlertTitle>

          <AlertDescription className='space-y-3'>
            <div>
              {gate.message ??
                'This app cannot be launched until required sandbox tests pass.'}
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
      </div>
    );
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
