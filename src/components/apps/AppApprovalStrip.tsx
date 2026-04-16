import { Button } from '@/components/ui/button.tsx';

export type PendingApproval = {
  kind: 'send_xch';
  appId: string;
  appName: string;
  requestId: string;
  summary: {
    address: string;
    amount: string;
    fee: string;
    memos: string[];
    autoSubmit: boolean;
  };
} | null;

interface Props {
  approval: PendingApproval;
  expanded: boolean;
  queuedApprovalCount: number;
  secondsLeft: number;
  onToggleExpanded: () => void;
  onApprove: () => void;
  onReject: () => void;
}

export function AppApprovalStrip({
  approval,
  expanded,
  queuedApprovalCount,
  secondsLeft,
  onToggleExpanded,
  onApprove,
  onReject,
}: Props) {
  if (!approval) {
    return null;
  }

  return (
    <div className='shrink-0 border-b bg-muted/40'>
      <div className='flex items-center justify-between gap-4 px-4 py-3'>
        <div className='min-w-0'>
          <div className='text-sm font-medium'>
            Transaction approval required
          </div>
          <div className='truncate text-xs text-muted-foreground'>
            {approval.appName}: send {approval.summary.amount} to{' '}
            {approval.summary.address}
          </div>
          <div className='text-xs text-muted-foreground'>
            Expires in {secondsLeft}s
            {queuedApprovalCount > 0
              ? ` · ${queuedApprovalCount} more approval${queuedApprovalCount === 1 ? '' : 's'} pending`
              : ''}
          </div>
        </div>

        <div className='flex items-center gap-2'>
          <Button variant='outline' size='sm' onClick={onToggleExpanded}>
            {expanded ? 'Hide details' : 'Inspect'}
          </Button>
          <Button variant='outline' size='sm' onClick={onReject}>
            Reject
          </Button>
          <Button size='sm' onClick={onApprove}>
            Approve
          </Button>
        </div>
      </div>

      {expanded ? (
        <div className='border-t px-4 py-3'>
          <pre className='overflow-auto rounded-md bg-background p-3 text-xs'>
            {JSON.stringify(approval.summary, null, 2)}
          </pre>
        </div>
      ) : null}
    </div>
  );
}
