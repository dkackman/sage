import { Button } from '@/components/ui/button';

export type AppsLaunchpadContextMenuUpdateState =
  | 'idle'
  | 'checking'
  | 'up_to_date';

interface Props {
  open: boolean;
  x: number;
  y: number;
  busy: boolean;
  hasUpdate: boolean;
  isRunning: boolean;
  updateCheckState: AppsLaunchpadContextMenuUpdateState;
  clearDataBusy?: boolean;
  clearDataError?: string | null;
  onClose: () => void;
  onOpen: () => void;
  onCheckForUpdate: () => void;
  onUpdate: () => void;
  onChangePermissions: () => void;
  onClearData: () => void;
  onUninstall: () => void;
}

export function AppsLaunchpadContextMenu({
  open,
  x,
  y,
  busy,
  hasUpdate,
  isRunning,
  updateCheckState,
  clearDataBusy = false,
  clearDataError = null,
  onClose,
  onOpen,
  onCheckForUpdate,
  onUpdate,
  onChangePermissions,
  onClearData,
  onUninstall,
}: Props) {
  if (!open) {
    return null;
  }

  return (
    <>
      <div className='absolute inset-0 z-40' onClick={onClose} />

      <div
        className='absolute z-50 w-[260px] rounded-xl border bg-popover p-1 shadow-lg'
        style={{
          left: `${x}px`,
          top: `${y}px`,
        }}
        onClick={(event) => {
          event.stopPropagation();
        }}
      >
        <button
          type='button'
          className='flex w-full rounded-lg px-3 py-2 text-left text-sm hover:bg-muted'
          onClick={onOpen}
        >
          Open
        </button>

        {!hasUpdate ? (
          <button
            type='button'
            className='flex w-full rounded-lg px-3 py-2 text-left text-sm hover:bg-muted disabled:opacity-50'
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
          </button>
        ) : (
          <button
            type='button'
            className='flex w-full rounded-lg px-3 py-2 text-left text-sm hover:bg-muted disabled:opacity-50'
            disabled={busy || clearDataBusy}
            onClick={onUpdate}
          >
            {isRunning ? 'Update and reopen' : 'Update'}
          </button>
        )}

        <div className='my-1 h-px bg-border' />

        <Button
          variant='ghost'
          className='h-auto w-full justify-start rounded-lg px-3 py-2 text-sm'
          disabled={busy || clearDataBusy}
          onClick={onChangePermissions}
        >
          Change permissions
        </Button>

        <button
          type='button'
          className='flex w-full rounded-lg px-3 py-2 text-left text-sm hover:bg-muted disabled:opacity-50'
          disabled={busy || clearDataBusy}
          onClick={onClearData}
        >
          {clearDataBusy
            ? isRunning
              ? 'Clearing data and reopening...'
              : 'Clearing data...'
            : isRunning
              ? 'Clear data and reopen'
              : 'Clear data'}
        </button>

        {clearDataError ? (
          <div className='px-3 py-2 text-xs text-destructive break-words'>
            {clearDataError}
          </div>
        ) : null}

        <button
          type='button'
          className='flex w-full rounded-lg px-3 py-2 text-left text-sm text-destructive hover:bg-muted disabled:opacity-50'
          disabled={busy || clearDataBusy}
          onClick={onUninstall}
        >
          Uninstall
        </button>
      </div>
    </>
  );
}
