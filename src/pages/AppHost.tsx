import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { killRuntime } from '@/lib/apps/runtimeRegistry';
import { ArrowLeft } from 'lucide-react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Link, useNavigate, useParams } from 'react-router-dom';
import { useAppRuntimePresence } from '@/hooks/useAppRuntimePresence';
import { useApps } from '@/contexts/AppsContext';
import { useAppPendingApprovals } from '@/hooks/useAppPendingApprovals.ts';
import { useAppEmbeddedRuntime } from '@/hooks/useAppEmbeddedRuntime.ts';
import { AppApprovalBanner } from '@/components/apps/AppApprovalBanner.tsx';

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
  const navigate = useNavigate();

  const {
    getApp,
    loading,
    checkForUpdate,
    performAppUpdate,
    updateAvailability,
    busyAppIds,
  } = useApps();

  const app = getApp(appId);
  const isRunning = useAppRuntimePresence(appId);
  const [applyingUpdate, setApplyingUpdate] = useState(false);

  const {
    currentApproval,
    queuedApprovalCount,
    currentApprovalSecondsLeft,
    requestApproval,
    approveCurrentApproval,
    rejectCurrentApproval,
  } = useAppPendingApprovals();

  const checkingUpdate = busyAppIds[appId] ?? false;
  const updatePreview = updateAvailability[appId] ?? null;

  const sourceDisplayUrl = useMemo(() => {
    if (!app) {
      return null;
    }

    return app.source.kind === 'url'
      ? app.source.appUrl
      : `sage-app://${app.id}`;
  }, [app]);

  const { scheduleSyncBounds } = useAppEmbeddedRuntime({
    app,
    containerRef,
    requestApproval,
  });

  const handleUpdateAndReopen = useCallback(async () => {
    if (!app) {
      return;
    }

    try {
      setApplyingUpdate(true);

      await performAppUpdate(app.id, {
        restartIfRunning: true,
        visibleAfterRestart: true,
      });

      scheduleSyncBounds(app.id);
    } finally {
      setApplyingUpdate(false);
    }
  }, [app, performAppUpdate, scheduleSyncBounds]);

  useEffect(() => {
    if (!app || app.source?.kind !== 'url') {
      return;
    }

    let cancelled = false;

    const check = async () => {
      try {
        await checkForUpdate(app.id, false);
      } catch {
        // keep quiet
      }
    };

    void check();

    const intervalId = window.setInterval(
      () => {
        if (!cancelled) {
          void check();
        }
      },
      10 * 60 * 1000,
    );

    return () => {
      cancelled = true;
      window.clearInterval(intervalId);
    };
  }, [app, checkForUpdate]);

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
      <div className='flex h-full min-h-0 w-full flex-col'>
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

        <AppApprovalBanner
          currentApproval={currentApproval}
          queuedApprovalCount={queuedApprovalCount}
          currentApprovalSecondsLeft={currentApprovalSecondsLeft}
          onApprove={approveCurrentApproval}
          onReject={rejectCurrentApproval}
        />

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
            App URL: {sourceDisplayUrl}
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
