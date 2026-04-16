import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { PendingApprovalItem } from '@/hooks/useAppPendingApprovals.ts';

interface Props {
  currentApproval: PendingApprovalItem | null;
  queuedApprovalCount: number;
  currentApprovalSecondsLeft: number;
  onApprove: () => void;
  onReject: () => void;
}

export function AppApprovalBanner({
  currentApproval,
  queuedApprovalCount,
  currentApprovalSecondsLeft,
  onApprove,
  onReject,
}: Props) {
  if (!currentApproval) {
    return null;
  }

  return (
    <Alert>
      <AlertTitle>Approval required</AlertTitle>
      <AlertDescription className='space-y-3'>
        <div className='text-sm'>
          App <strong>{currentApproval.request.app.name}</strong> wants to
          perform{' '}
          <span className='font-mono'>{currentApproval.request.kind}</span>.
        </div>

        <div className='text-xs text-muted-foreground'>
          Expires in {currentApprovalSecondsLeft}s
          {queuedApprovalCount > 0
            ? ` · ${queuedApprovalCount} more approval${queuedApprovalCount === 1 ? '' : 's'} pending`
            : ''}
        </div>

        {currentApproval.request.kind === 'send_xch' ? (
          <div className='rounded-md border p-3 text-xs font-mono whitespace-pre-wrap break-all'>
            {JSON.stringify(currentApproval.request.params, null, 2)}
          </div>
        ) : null}

        <div className='flex gap-2'>
          <Button variant='outline' onClick={onReject}>
            Reject
          </Button>

          <Button onClick={onApprove}>Approve</Button>
        </div>
      </AlertDescription>
    </Alert>
  );
}
