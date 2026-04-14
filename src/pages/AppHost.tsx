import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { useApps } from '@/hooks/useApps';
import { ArrowLeft, ExternalLink } from 'lucide-react';
import { Link, useParams } from 'react-router-dom';

function AppNotFound() {
  return (
    <div className='mx-auto w-full max-w-4xl p-4 md:p-6'>
      <Alert>
        <AlertTitle>App not found</AlertTitle>
        <AlertDescription>This app is not installed.</AlertDescription>
      </Alert>
    </div>
  );
}

export function AppHost() {
  const { appId = '' } = useParams();
  const { getApp } = useApps();

  const app = getApp(appId);

  if (!app) {
    return <AppNotFound />;
  }

  return (
    <div className='flex-1 overflow-auto'>
      <div className='mx-auto w-full max-w-7xl p-4 md:p-6 space-y-4 h-full'>
        <div className='flex items-center justify-between gap-4'>
          <Button asChild variant='ghost' className='pl-0'>
            <Link to='/apps'>
              <ArrowLeft className='h-4 w-4 mr-2' />
              Back to Apps
            </Link>
          </Button>

          <Button asChild variant='outline'>
            <a href={app.entry} target='_blank' rel='noreferrer'>
              <ExternalLink className='h-4 w-4 mr-2' />
              Open externally
            </a>
          </Button>
        </div>

        <div className='space-y-1'>
          <h1 className='text-2xl font-semibold tracking-tight'>{app.name}</h1>
          <p className='text-sm text-muted-foreground'>{app.description}</p>
        </div>

        <div className='rounded-xl border overflow-hidden bg-background h-[calc(100vh-220px)] min-h-[500px]'>
          <iframe
            key={app.id}
            src={app.entry}
            title={app.name}
            className='w-full h-full border-0'
            sandbox='allow-scripts allow-same-origin allow-forms allow-popups allow-downloads'
          />
        </div>
      </div>
    </div>
  );
}

