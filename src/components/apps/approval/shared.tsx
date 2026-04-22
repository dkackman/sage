import React from 'react';

export function ApprovalMetaPill({ children }: { children: React.ReactNode }) {
  return (
    <span className='rounded-full border px-2 py-0.5 text-[10px] uppercase tracking-wide text-muted-foreground'>
      {children}
    </span>
  );
}

export function ApprovalDetailRow({
  label,
  value,
  mono = false,
  breakAll = false,
}: {
  label: string;
  value: React.ReactNode;
  mono?: boolean;
  breakAll?: boolean;
}) {
  return (
    <div className='grid grid-cols-[90px_minmax(0,1fr)] gap-3 text-sm'>
      <div className='text-muted-foreground'>{label}</div>
      <div
        className={[
          mono ? 'font-mono text-xs' : '',
          breakAll ? 'break-all' : 'truncate',
        ].join(' ')}
      >
        {value}
      </div>
    </div>
  );
}
