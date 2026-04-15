import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Trash2 } from 'lucide-react';
import { useMemo } from 'react';
import { useNavigate } from 'react-router-dom';
import { InstalledSageApp } from '@/bindings.ts';

interface Props {
  app: InstalledSageApp;
  onUninstall: () => Promise<void>;
}

function permissionLabel(key: keyof InstalledSageApp['permissions']) {
  switch (key) {
    case 'network':
      return 'Network';
    case 'persistentStorage':
      return 'Persistent storage';
    default:
      return key;
  }
}

export function InstalledAppCard({ app, onUninstall }: Props) {
  const navigate = useNavigate();

  const iconSrc = useMemo(() => {
    return `sage-app://${app.id}/icon.png`;
  }, [app.id]);

  return (
    <Card>
      <CardHeader className='flex flex-row items-start justify-between space-y-0 gap-4'>
        <div className='space-y-2 min-w-0'>
          <CardTitle className='flex items-center gap-3'>
            <img
              src={iconSrc}
              alt=''
              className='h-8 w-8 rounded-md border object-cover'
            />
            <span>{app.name}</span>
            <Badge variant='outline'>installed</Badge>
          </CardTitle>

          <div className='text-sm text-muted-foreground'>v{app.version}</div>

          <div className='text-xs text-muted-foreground break-all'>
            Install dir: {app.installDir}
          </div>
        </div>

        <div className='flex items-center gap-2 shrink-0'>
          <Button onClick={() => navigate(`/apps/${app.id}`)}>Open</Button>
          <Button variant='outline' onClick={() => void onUninstall()}>
            <Trash2 className='h-4 w-4 mr-2' />
            Remove
          </Button>
        </div>
      </CardHeader>

      <CardContent className='space-y-3'>
        <div className='flex flex-wrap gap-2'>
          {(
            Object.entries(app.permissions) as [
              keyof InstalledSageApp['permissions'],
              boolean,
            ][]
          )
            .filter(([, allowed]) => allowed)
            .map(([permission]) => (
              <Badge key={permission} variant='outline'>
                {permissionLabel(permission)}
              </Badge>
            ))}
        </div>
      </CardContent>
    </Card>
  );
}

