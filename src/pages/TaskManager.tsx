import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { useAppRuntimes } from '@/hooks/useAppRuntimes';
import {
  focusRuntime,
  hideRuntime,
  killRuntime,
} from '@/lib/apps/runtimeRegistry';
import { ArrowLeft } from 'lucide-react';
import { Link, useLocation, useNavigate } from 'react-router-dom';

function formatDuration(ms: number) {
  const s = Math.floor(ms / 1000);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  return [h, m, sec].map((v) => String(v).padStart(2, '0')).join(':');
}

export function TaskManager() {
  const runtimes = useAppRuntimes();
  const navigate = useNavigate();
  const location = useLocation();

  return (
    <div className='mx-auto w-full max-w-6xl p-4 md:p-6'>
      <Card>
        <CardHeader className='flex flex-row items-center justify-between gap-4 space-y-0'>
          <div>
            <CardTitle>Task Manager</CardTitle>
          </div>

          <Button asChild variant='ghost' className='pl-0'>
            <Link to='/apps'>
              <ArrowLeft className='mr-2 h-4 w-4' />
              Back to Apps
            </Link>
          </Button>
        </CardHeader>

        <CardContent className='space-y-4'>
          {runtimes.length === 0 ? (
            <div className='text-sm text-muted-foreground'>
              No running apps.
            </div>
          ) : (
            <div className='space-y-3'>
              {runtimes.map((runtime) => (
                <div key={runtime.runtimeId} className='rounded-xl border p-4'>
                  <div className='flex items-start justify-between gap-4'>
                    <div className='min-w-0 space-y-1'>
                      <div className='font-medium'>{runtime.appName}</div>
                      <div className='break-all text-xs text-muted-foreground'>
                        {runtime.appId}
                      </div>
                      <div className='text-xs text-muted-foreground'>
                        state={runtime.state} · mode={runtime.mode} · uptime=
                        {formatDuration(Date.now() - runtime.startedAt)}
                      </div>
                      <div className='text-xs text-muted-foreground'>
                        requests={runtime.inFlightRequestCount} · batches=
                        {runtime.activeBatchCount} · sockets=
                        {runtime.activeSocketCount}
                      </div>
                    </div>

                    <div className='flex shrink-0 gap-2'>
                      <Button
                        variant='outline'
                        onClick={() => {
                          void focusRuntime(runtime.appId).then(() => {
                            navigate(`/apps/${runtime.appId}`);
                          });
                        }}
                      >
                        Focus
                      </Button>

                      <Button
                        variant='outline'
                        onClick={() => {
                          void hideRuntime(runtime.appId).then(() => {
                            if (
                              location.pathname === `/apps/${runtime.appId}`
                            ) {
                              navigate('/apps');
                            }
                          });
                        }}
                      >
                        Hide
                      </Button>

                      <Button
                        variant='destructive'
                        onClick={() => {
                          void killRuntime(runtime.appId).then(() => {
                            if (
                              location.pathname === `/apps/${runtime.appId}`
                            ) {
                              navigate('/apps');
                            }
                          });
                        }}
                      >
                        Kill
                      </Button>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
