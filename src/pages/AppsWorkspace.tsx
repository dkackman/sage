import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import {
  AppApprovalStrip,
  type PendingApproval,
} from '@/components/apps/AppApprovalStrip.tsx';
import {
  AppTaskBar,
  type AppTaskBarTab,
} from '@/components/apps/AppTaskBar.tsx';
import { useApps } from '@/contexts/AppsContext.tsx';
import { useAppPendingApprovals } from '@/hooks/useAppPendingApprovals.ts';
import { useAppRuntimes } from '@/hooks/useAppRuntimes';
import { useBridgeHost } from '@/hooks/useBridgeHost';
import { focusRuntime, killRuntime } from '@/lib/apps/runtimeRegistry';
import { useCallback, useEffect, useMemo, useState } from 'react';
import {
  Outlet,
  useNavigate,
  useOutletContext,
  useParams,
} from 'react-router-dom';

export interface AppsWorkspaceOutletContext {
  requestApproval: ReturnType<typeof useAppPendingApprovals>['requestApproval'];
}

export function useAppsWorkspaceOutletContext() {
  return useOutletContext<AppsWorkspaceOutletContext>();
}

export function AppsWorkspace() {
  const { appId } = useParams();
  const navigate = useNavigate();
  const runtimes = useAppRuntimes();

  const { getApp, updateAvailability, busyAppIds, performAppUpdate } =
    useApps();

  const {
    currentApproval,
    queuedApprovalCount,
    currentApprovalSecondsLeft,
    requestApproval,
    approveCurrentApproval,
    rejectCurrentApproval,
  } = useAppPendingApprovals();

  const [approvalExpanded, setApprovalExpanded] = useState(false);
  const [applyingUpdate, setApplyingUpdate] = useState(false);
  const [tabOrder, setTabOrder] = useState<string[]>([]);

  useBridgeHost({ requestApproval });

  useEffect(() => {
    setTabOrder((prev) => {
      const runtimeIds = runtimes.map((runtime) => runtime.appId);

      const kept = prev.filter((appId) => runtimeIds.includes(appId));
      const added = runtimeIds.filter((appId) => !kept.includes(appId));

      return [...kept, ...added];
    });
  }, [runtimes]);

  const activeApp = appId ? getApp(appId) : null;
  const activeUpdatePreview = activeApp
    ? updateAvailability[activeApp.id]
    : null;
  const activeBusy = activeApp ? (busyAppIds[activeApp.id] ?? false) : false;

  useEffect(() => {
    setApprovalExpanded(false);
  }, [currentApproval?.id]);

  const tabs = useMemo<AppTaskBarTab[]>(() => {
    const runtimeByAppId = new Map(
      runtimes.map((runtime) => [runtime.appId, runtime]),
    );

    return tabOrder
      .map((appId) => {
        const runtime = runtimeByAppId.get(appId);
        if (!runtime) {
          return null;
        }

        const installedApp = getApp(runtime.appId);

        return {
          appId: runtime.appId,
          name: installedApp?.name ?? runtime.appName,
          iconSrc: installedApp
            ? `sage-app://${installedApp.id}/${installedApp.iconFile}`
            : null,
          isActive: runtime.appId === activeApp?.id,
        };
      })
      .filter((tab): tab is AppTaskBarTab => tab !== null);
  }, [runtimes, tabOrder, getApp, activeApp?.id]);

  const approvalStripData = useMemo<PendingApproval>(() => {
    if (!currentApproval) {
      return null;
    }

    if (currentApproval.request.kind === 'send_xch') {
      return {
        kind: 'send_xch',
        appId: currentApproval.request.app.id,
        appName: currentApproval.request.app.name,
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

  const handleApplyUpdate = useCallback(async () => {
    if (!activeApp) {
      return;
    }

    try {
      setApplyingUpdate(true);

      await performAppUpdate(activeApp.id, {
        restartIfRunning: true,
        visibleAfterRestart: true,
      });
    } finally {
      setApplyingUpdate(false);
    }
  }, [activeApp, performAppUpdate]);

  return (
    <div className='flex h-full min-h-0 w-full flex-col overflow-hidden'>
      <AppTaskBar
        tabs={tabs}
        onOpenApps={() => {
          navigate('/apps');
        }}
        onSelectApp={(targetAppId) => {
          void focusRuntime(targetAppId).then(() => {
            navigate(`/apps/${targetAppId}`);
          });
        }}
        onCloseApp={(targetAppId) => {
          void killRuntime(targetAppId).then(() => {
            if (targetAppId === activeApp?.id) {
              navigate('/apps');
            }
          });
        }}
        onReorderTabs={setTabOrder}
      />

      {activeApp ? (
        <AppApprovalStrip
          approval={approvalStripData}
          expanded={approvalExpanded}
          queuedApprovalCount={queuedApprovalCount}
          secondsLeft={currentApprovalSecondsLeft}
          onToggleExpanded={() => {
            setApprovalExpanded((prev) => !prev);
          }}
          onApprove={approveCurrentApproval}
          onReject={rejectCurrentApproval}
        />
      ) : null}

      {activeApp?.source?.kind === 'url' && activeUpdatePreview ? (
        <Alert className='shrink-0 rounded-none border-x-0 border-t-0'>
          <AlertTitle>New version available</AlertTitle>
          <AlertDescription className='flex items-center justify-between gap-4'>
            <span>
              Version {activeUpdatePreview.manifest.version} is available for{' '}
              {activeApp.name}.
            </span>

            <Button
              variant='outline'
              disabled={activeBusy || applyingUpdate}
              onClick={() => {
                void handleApplyUpdate();
              }}
            >
              {applyingUpdate ? 'Updating...' : 'Update and reopen'}
            </Button>
          </AlertDescription>
        </Alert>
      ) : null}

      <div className='flex-1 min-h-0 overflow-hidden'>
        <Outlet context={{ requestApproval }} />
      </div>
    </div>
  );
}
