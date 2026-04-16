import { Button } from '@/components/ui/button.tsx';
import { Link } from 'react-router-dom';
import { ArrowLeft } from 'lucide-react';

export function AppTaskBar({
  appName,
  onExit,
}: {
  appName: string;
  onExit: () => void;
}) {
  return (
    <div className='flex h-12 shrink-0 items-center justify-between border-b bg-background px-3'>
      <div className='flex items-center gap-3'>
        <div className='text-sm font-medium'>{appName}</div>
      </div>

      <div className='flex items-center gap-2'>
        <Button variant='ghost' size='sm' asChild>
          <Link to='/apps'>
            <ArrowLeft className='mr-2 h-4 w-4' />
            Apps
          </Link>
        </Button>

        <Button variant='destructive' size='sm' onClick={onExit}>
          Exit App
        </Button>
      </div>
    </div>
  );
}
