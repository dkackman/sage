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
import { useAppRuntimes } from '@/hooks/useAppRuntimes';
import { focusRuntime, killRuntime } from '@/lib/apps/runtimeRegistry';
import { formatAppError } from '@/lib/apps/formatAppError';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { Outlet, useNavigate, useParams } from 'react-router-dom';
import {
  commands,
  InstalledSageApp,
  SageAppUrlPreview,
  SageGrantedPermissions,
} from '@/bindings';
import { AppUpdateDialog } from '@/components/apps/AppUpdateDialog.tsx';
import { getAppUpdatePermissionsDelta } from '@/lib/apps/updatePermissionsDelta.ts';
import { AppDonationStrip } from '@/components/apps/AppDonationStrip.tsx';

export function AppsWorkspace() {
  const { appId } = useParams();
  const navigate = useNavigate();
  const runtimes = useAppRuntimes();

  const {
    getApp,
    updateAvailability,
    busyAppIds,
    performAppUpdate,
    currentApproval,
    queuedApprovalCount,
    currentApprovalSecondsLeft,
    approveCurrentApproval,
    rejectCurrentApproval,
  } = useApps();

  const [approvalExpanded, setApprovalExpanded] = useState(false);
  const [applyingUpdate, setApplyingUpdate] = useState(false);
  const [tabOrder, setTabOrder] = useState<string[]>([]);
  const [updateDialogOpen, setUpdateDialogOpen] = useState(false);
  const [updateDialogError, setUpdateDialogError] = useState<string | null>(
    null,
  );

  useEffect(() => {
    setTabOrder((prev) => {
      const runtimeIds = runtimes.map((runtime) => runtime.appId);
      const kept = prev.filter((appId) => runtimeIds.includes(appId));
      const added = runtimeIds.filter((appId) => !kept.includes(appId));
      return [...kept, ...added];
    });
  }, [runtimes]);

  const activeApp: InstalledSageApp | null = appId
    ? (getApp(appId) ?? null)
    : null;
  const activeUpdatePreview: SageAppUrlPreview | null = activeApp
    ? (updateAvailability[activeApp.id] ?? null)
    : null;
  const activeBusy = activeApp ? (busyAppIds[activeApp.id] ?? false) : false;
  const [donationOpen, setDonationOpen] = useState(false);
  const activeManifest = activeApp?.activeSnapshot.manifest;

  const hasDonation = !!activeManifest?.donation?.address;

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
            ? `sage-app://${installedApp.originId}/${installedApp.iconFile}`
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

    if (currentApproval.request.kind === 'capability_grant') {
      return {
        kind: 'capability_grant',
        appId: currentApproval.request.app.id,
        appName: currentApproval.request.app.name,
        requestId: currentApproval.request.requestId,
        capability: currentApproval.request.capability,
      };
    }

    if (currentApproval.request.kind === 'network_whitelist_grant') {
      return {
        kind: 'network_whitelist_grant',
        appId: currentApproval.request.app.id,
        appName: currentApproval.request.app.name,
        requestId: currentApproval.request.requestId,
        entry: {
          scheme: currentApproval.request.entry.scheme,
          host: currentApproval.request.entry.host,
        },
      };
    }

    return null;
  }, [currentApproval]);

  const handleConfirmUpdate = useCallback(
    async (nextGrantedPermissions: SageGrantedPermissions) => {
      if (!activeApp) {
        return;
      }

      try {
        setApplyingUpdate(true);
        setUpdateDialogError(null);

        await performAppUpdate(activeApp.id, nextGrantedPermissions, {
          restartIfRunning: true,
          visibleAfterRestart: true,
        });

        setUpdateDialogOpen(false);
      } catch (err) {
        setUpdateDialogError(formatAppError(err));
      } finally {
        setApplyingUpdate(false);
      }
    },
    [activeApp, performAppUpdate],
  );

  const handleReviewOrApplyUpdate = useCallback(async () => {
    if (!activeApp || !activeUpdatePreview) {
      return;
    }

    const delta = getAppUpdatePermissionsDelta(activeApp, activeUpdatePreview);
    console.log('update delta', delta);

    if (!delta.requiresUserReview) {
      setUpdateDialogOpen(false);
      setUpdateDialogError(null);
      await handleConfirmUpdate(delta.nextGrantedPermissions);
      return;
    }

    setUpdateDialogError(null);
    setUpdateDialogOpen(true);
  }, [activeApp, activeUpdatePreview, handleConfirmUpdate]);

  return (
    <div className='flex h-full min-h-0 w-full flex-col overflow-hidden'>
      <AppTaskBar
        tabs={tabs}
        activeAppId={activeApp?.id ?? null}
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
        activeAppHasDonation={hasDonation}
        onOpenDonation={() => setDonationOpen((v) => !v)}
      />

      {activeApp &&
      currentApproval &&
      currentApproval.request.app.id === activeApp.id ? (
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
      {donationOpen && activeApp && activeManifest?.donation ? (
        <AppDonationStrip
          appName={activeApp.name}
          authorName={activeManifest.author?.name}
          authorAvatarSrc={
            activeManifest.author?.avatar
              ? `sage-app://${activeApp.originId}/${activeManifest.author.avatar}`
              : null
          }
          donationAddress={activeManifest.donation.address}
          onSend={(amountMojos) => {
            if (!activeManifest.donation) {
              return;
            }
            void commands.sendXch({
              address: activeManifest?.donation.address,
              amount: amountMojos,
              fee: '0',
              memos: [],
              auto_submit: false,
            });
          }}
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
                void handleReviewOrApplyUpdate();
              }}
            >
              {applyingUpdate ? 'Updating...' : 'Review update'}
            </Button>
          </AlertDescription>
        </Alert>
      ) : null}

      <div className='flex-1 min-h-0 overflow-hidden'>
        <Outlet />
      </div>

      <AppUpdateDialog
        open={updateDialogOpen}
        app={activeApp}
        preview={activeUpdatePreview}
        submitting={applyingUpdate}
        error={updateDialogError}
        onCancel={() => {
          if (!applyingUpdate) {
            setUpdateDialogOpen(false);
            setUpdateDialogError(null);
          }
        }}
        onConfirm={(nextGranted) => {
          void handleConfirmUpdate(nextGranted);
        }}
      />
    </div>
  );
}
