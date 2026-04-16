import { Button } from '@/components/ui/button.tsx';

export type PendingApproval = {
  kind: 'send_xch';
  appId: string;
  requestId: string;
  summary: {
    address: string;
    amount: string;
    fee: string;
    memos: string[];
    autoSubmit: boolean;
  };
} | null;

export function AppApprovalStrip({
  approval,
  expanded,
  onToggleExpanded,
  onApprove,
  onReject,
}: {
  approval: PendingApproval;
  expanded: boolean;
  onToggleExpanded: () => void;
  onApprove: () => void;
  onReject: () => void;
}) {
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
            Send {approval.summary.amount} to {approval.summary.address}
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
