import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { LayoutGrid, Menu } from 'lucide-react';
import { useState } from 'react';

interface Props {
  showSandboxDebugUi: boolean;
  sandboxTestsRunning: boolean;
  onTaskManager: () => void;
  onRerunSandboxTests: () => void;
  onClose?: () => void;
}

export function AppsPageActionsMenu({
  showSandboxDebugUi,
  sandboxTestsRunning,
  onTaskManager,
  onRerunSandboxTests,
  onClose,
}: Props) {
  const [open, setOpen] = useState(false);

  function handleOpenChange(nextOpen: boolean) {
    setOpen(nextOpen);
    if (!nextOpen) {
      onClose?.();
    }
  }

  return (
    <DropdownMenu open={open} onOpenChange={handleOpenChange}>
      <DropdownMenuTrigger asChild>
        <Button variant='outline' size='icon' aria-label='Open apps actions'>
          <Menu className='h-4 w-4' />
        </Button>
      </DropdownMenuTrigger>

      <DropdownMenuContent align='end' className='w-56'>
        <DropdownMenuItem onClick={onTaskManager}>
          <LayoutGrid className='mr-2 h-4 w-4' />
          Task Manager
        </DropdownMenuItem>

        {showSandboxDebugUi ? (
          <>
            <DropdownMenuSeparator />
            <DropdownMenuItem
              disabled={sandboxTestsRunning}
              onClick={onRerunSandboxTests}
            >
              {sandboxTestsRunning
                ? 'Running sandbox tests...'
                : 'Re-run sandbox tests'}
            </DropdownMenuItem>
          </>
        ) : null}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
