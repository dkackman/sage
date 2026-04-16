import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { killRuntime } from '@/lib/apps/runtimeRegistry';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { useAppRuntimePresence } from '@/hooks/useAppRuntimePresence';
import { useApps } from '@/contexts/AppsContext';
import { useAppPendingApprovals } from '@/hooks/useAppPendingApprovals.ts';
import { useAppEmbeddedRuntime } from '@/hooks/useAppEmbeddedRuntime.ts';
import { AppTaskBar } from '@/components/apps/AppTaskBar.tsx';
import {
  AppApprovalStrip,
  type PendingApproval,
} from '@/components/apps/AppApprovalStrip.tsx';

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
  const [approvalExpanded, setApprovalExpanded] = useState(false);

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

  const approvalStripData = useMemo<PendingApproval>(() => {
    if (!currentApproval) {
      return null;
    }

    if (currentApproval.request.kind === 'send_xch') {
      return {
        kind: 'send_xch',
        appId: currentApproval.request.app.id,
        requestId: currentApproval.request.requestId,
        summary: {
          address: currentApproval.request.params.address,
          amount: String(currentApproval.request.params.amount),
          fee: String(currentApproval.request.params.fee),
          memos: currentApproval.request.params.memos ?? [],
          autoSubmit: false,
        },
      };
    }

    return null;
  }, [currentApproval]);

  useEffect(() => {
    setApprovalExpanded(false);
  }, [currentApproval?.id]);

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
        <AppTaskBar
          appName={app.name}
          onExit={() => {
            void killRuntime(app.id).then(() => {
              navigate('/apps');
            });
          }}
        />

        <AppApprovalStrip
          approval={approvalStripData}
          expanded={approvalExpanded}
          onToggleExpanded={() => {
            setApprovalExpanded((prev) => !prev);
          }}
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

        <div className='flex-1 min-h-0'>
          <div
            ref={containerRef}
            className='h-full w-full overflow-hidden bg-background'
          />
        </div>
      </div>
    </div>
  );
}
