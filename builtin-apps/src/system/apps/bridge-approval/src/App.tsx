import React, { useEffect, useMemo, useState } from 'react';
import { AppIcon, appIconFromCommonView, AppModalShell } from 'sage-app-ui';
import {
  useSageSystemClient,
  type PendingBridgeApprovalView,
  type SageAppRuntimeRecordView,
} from 'sage-system-app-sdk';
import { Clock } from 'lucide-react';
import { AppApprovalBody } from './approval/AppApprovalBody';

function appNameFromRuntime(
  runtime: SageAppRuntimeRecordView | null,
): string | null {
  return runtime?.app.common.activeSnapshot.manifest.name ?? null;
}

function appIconFromRuntime(
  runtime: SageAppRuntimeRecordView | null,
): AppIcon | null {
  if (!runtime) return null;
  return appIconFromCommonView(runtime.app.common);
}

function appIdFromRuntime(
  runtime: SageAppRuntimeRecordView | null,
): string | null {
  return runtime?.app.common.identity.id ?? null;
}

function titleForApproval(approval: PendingBridgeApprovalView) {
  switch (approval.approval.kind) {
    case 'sendXch':
      return 'Approve XCH transaction';
    case 'getSecretKey':
      return 'Approve secret key access';
    case 'signCoinSpends':
      return 'Approve coin-spend signatures';
    case 'signMessage':
      return 'Approve message signature';
    case 'capabilityGrant':
      return 'Approve permission grant';
    case 'networkWhitelistGrant':
      return 'Approve network access';
  }
}

function formatCountdown(expiresAt: number, now: number) {
  const seconds = Math.max(0, Math.ceil((expiresAt - now) / 1000));
  return seconds <= 0 ? 'Expires now' : `Expires in ${seconds}s`;
}

function queueText(count: number) {
  if (count <= 0) return null;
  return `+${count} pending`;
}

function MetaPill({ children }: { children: React.ReactNode }) {
  return (
    <span className='inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[10px] uppercase tracking-wide text-muted-foreground'>
      {children}
    </span>
  );
}

export function App() {
  const sage = useSageSystemClient();

  const [approvals, setApprovals] = useState<PendingBridgeApprovalView[]>([]);
  const [activeAppId, setActiveAppId] = useState<string | null>(null);
  const [activeAppName, setActiveAppName] = useState<string | null>(null);
  const [activeAppIcon, setActiveAppIcon] = useState<AppIcon | null>(null);
  const [expanded, setExpanded] = useState(false);
  const [working, setWorking] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const [now, setNow] = useState(() => Date.now());
  const [error, setError] = useState<string | null>(null);
  const [password, setPassword] = useState('');
  const [passwordError, setPasswordError] = useState<string | null>(null);
  // Set when the host tells us an approval needs a password that its queued
  // hint did not predict, so the field appears even on a stale view.
  const [passwordForced, setPasswordForced] = useState(false);

  async function refreshActiveRuntime() {
    const active = await sage.runtimeManager.getActiveTaskbarRuntime();

    setActiveAppId(appIdFromRuntime(active));
    setActiveAppName(appNameFromRuntime(active));
    setActiveAppIcon(appIconFromRuntime(active));

    return active;
  }

  useEffect(() => {
    const id = window.setInterval(() => setNow(Date.now()), 250);
    return () => window.clearInterval(id);
  }, []);

  useEffect(() => {
    async function refreshInitialState() {
      try {
        const [pending, active] = await Promise.all([
          sage.bridgeApprovals.listPending(),
          sage.runtimeManager.getActiveTaskbarRuntime(),
        ]);

        setApprovals(pending);
        setActiveAppId(appIdFromRuntime(active));
        setActiveAppName(appNameFromRuntime(active));
        setActiveAppIcon(appIconFromRuntime(active));
      } catch (err) {
        console.error('[approval] refreshInitialState failed', err);
        setError(err instanceof Error ? err.message : String(err));
      } finally {
        setLoaded(true);
      }
    }

    void refreshInitialState();
  }, [sage]);

  useEffect(() => {
    const offApprovals = sage.bridgeApprovals.onChanged((event) => {
      setApprovals(event.approvals);
    });

    const offActiveRuntime = sage.runtimeManager.onActiveTaskbarRuntimeChanged(
      () => {
        void refreshActiveRuntime().catch((err) => {
          console.error('[approval] failed to refresh active runtime', err);
        });
      },
    );

    return () => {
      offApprovals();
      offActiveRuntime();
    };
  }, [sage]);

  const activeApprovals = useMemo(() => {
    if (!activeAppId) return [];

    return approvals
      .filter((item) => item.appId === activeAppId)
      .sort((a, b) => a.expiresAtMs - b.expiresAtMs);
  }, [approvals, activeAppId]);

  const activeApproval = activeApprovals[0] ?? null;
  const pendingForActiveAppCount = activeApprovals.length;
  const queuedApprovalCount = Math.max(0, pendingForActiveAppCount - 1);

  const countdownText = activeApproval
    ? formatCountdown(activeApproval.expiresAtMs, now)
    : null;

  // Never carry a typed password across approvals.
  useEffect(() => {
    setExpanded(false);
    setError(null);
    setPassword('');
    setPasswordError(null);
    setPasswordForced(false);
  }, [activeApproval?.approvalId]);

  const needsPassword =
    (activeApproval?.requiresPassword ?? false) || passwordForced;

  async function resolve(approved: boolean) {
    if (!activeApproval || working) return;
    if (approved && needsPassword && password.length === 0) return;

    setWorking(true);
    setError(null);

    try {
      const result = await sage.bridgeApprovals.resolve({
        approvalId: activeApproval.approvalId,
        approved,
        reason: approved ? null : 'User denied the request',
        password: approved && needsPassword ? password : null,
      });

      switch (result.kind) {
        case 'wrongPassword':
          // The approval is still queued; keep the card up for another try.
          setPassword('');
          setPasswordError(
            result.attemptsRemaining === 1
              ? 'Incorrect password. 1 attempt remaining.'
              : `Incorrect password. ${result.attemptsRemaining} attempts remaining.`,
          );
          break;

        case 'passwordRequired':
          // The queued hint was stale — this wallet is protected after all.
          setPasswordForced(true);
          setPassword('');
          setPasswordError('This wallet requires its password.');
          break;

        case 'tooManyAttempts':
          setPassword('');
          setPasswordError(null);
          setError('Too many incorrect password attempts. Request rejected.');
          break;

        case 'resolved':
          setPassword('');
          setPasswordError(null);
          break;
      }

      setApprovals(await sage.bridgeApprovals.listPending());
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setWorking(false);
    }
  }

  if (!loaded) {
    return null;
  }

  if (!activeApproval) {
    return (
      <AppModalShell
        title='No approval pending'
        appName='Bridge Approval'
        appIcon={null}
        footer={
          <div className='flex justify-end'>
            <button
              type='button'
              onClick={() => {
                void sage.runtimeManager.closeSelf();
              }}
              className='rounded-md border border-border px-3 py-2 text-sm hover:bg-muted'
            >
              Close
            </button>
          </div>
        }
      >
        <div className='text-sm text-muted-foreground'>
          There are no approval requests to review.
        </div>
      </AppModalShell>
    );
  }

  if (!activeAppId || !activeAppName) {
    return (
      <AppModalShell
        title='Approval state issue'
        appName='Bridge Approval'
        appIcon={null}
      >
        <div className='rounded-lg border border-destructive/40 bg-destructive/10 p-3 text-sm text-destructive'>
          Approval exists, but no active taskbar app was resolved.
        </div>
      </AppModalShell>
    );
  }

  const moreText = queueText(queuedApprovalCount);

  return (
    <AppModalShell
      title={titleForApproval(activeApproval)}
      appName={activeAppName}
      appIcon={activeAppIcon}
      footer={
        <div className='flex items-center justify-end gap-3'>
          <div className='flex items-center gap-2'>
            <button
              type='button'
              disabled={working}
              onClick={() => void resolve(false)}
              className='rounded-md border border-border px-3 py-1.5 text-sm hover:bg-muted disabled:opacity-50'
            >
              Reject
            </button>

            <button
              type='button'
              disabled={working || (needsPassword && password.length === 0)}
              onClick={() => void resolve(true)}
              className='rounded-md bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground hover:opacity-90 disabled:opacity-50'
            >
              Approve
            </button>
          </div>
        </div>
      }
    >
      <div className='space-y-4'>
        <div className='flex flex-wrap items-center gap-2'>
          <div className='text-sm font-semibold'>Approval required</div>

          {countdownText ? (
            <MetaPill>
              <Clock className='h-3 w-3' />
              {countdownText}
            </MetaPill>
          ) : null}

          {moreText ? <MetaPill>{moreText}</MetaPill> : null}
        </div>

        <AppApprovalBody
          approval={activeApproval.approval}
          appName={activeAppName}
          expanded={expanded}
        />

        {needsPassword ? (
          <div className='space-y-1.5'>
            <label
              htmlFor='approval-password'
              className='text-xs font-medium uppercase tracking-wide text-muted-foreground'
            >
              Wallet password
            </label>

            <input
              id='approval-password'
              type='password'
              autoFocus
              autoComplete='off'
              spellCheck={false}
              value={password}
              disabled={working}
              onChange={(event) => {
                setPassword(event.target.value);
                setPasswordError(null);
              }}
              onKeyDown={(event) => {
                if (event.key === 'Enter' && password.length > 0) {
                  void resolve(true);
                }
              }}
              className='w-full rounded-md border border-border bg-background px-3 py-2 text-sm disabled:opacity-50'
              placeholder='Required to sign with this wallet'
            />

            {passwordError ? (
              <div className='text-xs text-destructive'>{passwordError}</div>
            ) : null}
          </div>
        ) : null}

        {error ? (
          <div className='rounded-lg border border-destructive/40 bg-destructive/10 p-2 text-sm text-destructive'>
            {error}
          </div>
        ) : null}
      </div>
    </AppModalShell>
  );
}
