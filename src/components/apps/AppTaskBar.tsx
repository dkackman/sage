import { Button } from '@/components/ui/button.tsx';
import { LayoutGrid, X } from 'lucide-react';
import clsx from 'clsx';

export interface AppTaskBarTab {
  appId: string;
  name: string;
  iconSrc: string | null;
  isActive: boolean;
}

interface Props {
  tabs: AppTaskBarTab[];
  onOpenApps: () => void;
  onSelectApp: (appId: string) => void;
  onCloseApp: (appId: string) => void;
}

export function AppTaskBar({
  tabs,
  onOpenApps,
  onSelectApp,
  onCloseApp,
}: Props) {
  return (
    <div className='flex h-12 shrink-0 items-end gap-2 border-b bg-muted/30 px-3 pt-2'>
      <Button
        variant='ghost'
        className='h-9 shrink-0 px-3'
        onClick={onOpenApps}
      >
        <LayoutGrid className='mr-2 h-4 w-4' />
        Apps
      </Button>

      <div className='flex min-w-0 flex-1 items-end gap-1 overflow-hidden'>
        {tabs.map((tab) => (
          <button
            key={tab.appId}
            type='button'
            onClick={() => onSelectApp(tab.appId)}
            className={clsx(
              'group flex h-9 min-w-[120px] max-w-[220px] flex-1 items-center gap-2 rounded-t-md border border-b-0 px-3 text-left transition-colors',
              tab.isActive
                ? 'bg-background'
                : 'bg-muted text-muted-foreground hover:bg-muted/80',
            )}
          >
            {tab.iconSrc ? (
              <img
                src={tab.iconSrc}
                alt=''
                className='h-4 w-4 shrink-0 rounded-sm'
              />
            ) : (
              <div className='flex h-4 w-4 shrink-0 items-center justify-center rounded-sm bg-border text-[10px] font-semibold'>
                {tab.name.slice(0, 1).toUpperCase()}
              </div>
            )}

            <span className='min-w-0 flex-1 truncate text-sm font-medium'>
              {tab.name}
            </span>

            <span
              className={clsx(
                'shrink-0',
                tab.isActive
                  ? 'opacity-100'
                  : 'opacity-0 transition-opacity group-hover:opacity-100',
              )}
            >
              <Button
                type='button'
                variant='ghost'
                size='icon'
                className='h-6 w-6'
                onClick={(event) => {
                  event.stopPropagation();
                  onCloseApp(tab.appId);
                }}
              >
                <X className='h-3.5 w-3.5' />
              </Button>
            </span>
          </button>
        ))}
      </div>
    </div>
  );
}
