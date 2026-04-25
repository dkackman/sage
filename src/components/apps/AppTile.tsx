import type { SageApp } from '@/bindings';
import type { SandboxLaunchDecision } from '@/lib/apps/sandboxPolicy';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { MoreVertical } from 'lucide-react';

export type AppTileUpdateCheckState = 'idle' | 'checking' | 'up_to_date';

interface Props {
  app: SageApp;
  launchDecision: SandboxLaunchDecision;
  busy: boolean;
  hasUpdate: boolean;
  isRunning: boolean;
  updateCheckState: AppTileUpdateCheckState;
  clearDataBusy?: boolean;
  clearDataError?: string | null;
  clearDataEnabled?: boolean;
  clearDataDisabledReason?: string | null;
  onOpen: () => void;
  onMenuClose?: () => void;
  onCheckForUpdate: () => void;
  onUpdate: () => void;
  onChangePermissions: () => void;
  onClearData: () => void;
  onUninstall: () => void;
}

export function AppTile({
  app,
  launchDecision,
  busy,
  hasUpdate,
  isRunning,
  updateCheckState,
  clearDataBusy = false,
  clearDataError = null,
  clearDataEnabled = true,
  clearDataDisabledReason = null,
  onOpen,
  onMenuClose,
  onCheckForUpdate,
  onUpdate,
  onChangePermissions,
  onClearData,
  onUninstall,
}: Props) {
  const iconSrc =
    app.kind === 'system'
      ? `sage-system-app://${app.common.originId}/${app.common.iconFile}`
      : `sage-app://${app.common.originId}/${app.common.iconFile}`;

  const isChecking =
    !launchDecision.allowed &&
    launchDecision.title === 'Sandbox tests are still running';

  const isBlocked = !launchDecision.allowed && !isChecking;
  const clearDataDisabled = busy || clearDataBusy || !clearDataEnabled;

  return (
    <div
      className='relative group flex flex-col items-center gap-2 rounded-2xl px-4 pt-2 pb-4 text-center transition-colors hover:bg-muted/50 cursor-pointer'
      onClick={() => {
        if (!launchDecision.allowed) return;
        onOpen();
      }}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          if (!launchDecision.allowed) return;
          onOpen();
        }
      }}
      role='button'
      tabIndex={0}
    >
      {isChecking || isBlocked ? (
        <div className='absolute inset-0 z-10 flex items-center justify-center rounded-2xl bg-background/55 backdrop-blur-[1px]'>
          {isChecking ? (
            <div className='flex flex-col items-center gap-2'>
              <div className='h-5 w-5 animate-spin rounded-full border-2 border-muted-foreground/30 border-t-muted-foreground' />
              <div className='text-xs text-muted-foreground'>Checking…</div>
            </div>
          ) : (
            <div className='px-3 text-center text-xs font-medium text-amber-600'>
              Blocked
            </div>
          )}
        </div>
      ) : null}

      <div className='absolute top-2 right-1'>
        <DropdownMenu
          onOpenChange={(open) => {
            if (!open) onMenuClose?.();
          }}
        >
          <DropdownMenuTrigger asChild>
            <Button
              variant='ghost'
              size='icon'
              onClick={(e) => e.stopPropagation()}
              aria-label='App options'
            >
              <MoreVertical className='h-5 w-5' aria-hidden='true' />
            </Button>
          </DropdownMenuTrigger>

          <DropdownMenuContent align='end' className='w-52'>
            <DropdownMenuItem onClick={onOpen}>Open</DropdownMenuItem>

            {!hasUpdate ? (
              <DropdownMenuItem
                disabled={
                  busy ||
                  clearDataBusy ||
                  updateCheckState === 'checking' ||
                  updateCheckState === 'up_to_date'
                }
                onClick={onCheckForUpdate}
              >
                {updateCheckState === 'checking'
                  ? 'Checking…'
                  : updateCheckState === 'up_to_date'
                    ? 'Up to date'
                    : 'Check for update'}
              </DropdownMenuItem>
            ) : (
              <DropdownMenuItem
                disabled={busy || clearDataBusy}
                onClick={onUpdate}
              >
                {isRunning ? 'Update and reopen' : 'Update'}
              </DropdownMenuItem>
            )}

            <DropdownMenuSeparator />

            <DropdownMenuItem
              disabled={busy || clearDataBusy}
              onClick={onChangePermissions}
            >
              Change permissions
            </DropdownMenuItem>

            <DropdownMenuItem
              disabled={clearDataDisabled}
              onClick={onClearData}
            >
              {clearDataBusy
                ? isRunning
                  ? 'Clearing data and reopening...'
                  : 'Clearing data...'
                : isRunning
                  ? 'Clear data and reopen'
                  : 'Clear data'}
            </DropdownMenuItem>

            {clearDataError ? (
              <div className='px-2 py-1 text-xs text-destructive break-words'>
                {clearDataError}
              </div>
            ) : !clearDataEnabled && clearDataDisabledReason ? (
              <div className='px-2 py-1 text-xs text-muted-foreground break-words'>
                {clearDataDisabledReason}
              </div>
            ) : null}

            <DropdownMenuSeparator />

            <DropdownMenuItem
              className='text-destructive focus:text-destructive'
              disabled={busy || clearDataBusy}
              onClick={onUninstall}
            >
              Uninstall
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>

      <div className='flex h-20 w-20 items-center justify-center overflow-hidden rounded-2xl border bg-background shadow-sm'>
        <img src={iconSrc} alt='' className='h-full w-full object-cover' />
      </div>

      <div className='min-w-0 w-full'>
        <div className='truncate text-sm font-medium'>{app.common.name}</div>
        <div className='truncate text-xs text-muted-foreground'>
          v{app.common.version}
        </div>

        {isBlocked ? (
          <div className='relative z-20 mt-1 text-xs text-amber-600'>
            {launchDecision.title}
          </div>
        ) : null}
      </div>
    </div>
  );
}
