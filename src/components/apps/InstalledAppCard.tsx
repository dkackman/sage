import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import type { InstalledSageApp } from '@/bindings';
import { Trash2 } from 'lucide-react';
import { useMemo } from 'react';
import { useNavigate } from 'react-router-dom';

interface Props {
  app: InstalledSageApp;
  onUninstall: () => Promise<void>;
}

export function InstalledAppCard({ app, onUninstall }: Props) {
  const navigate = useNavigate();

  const iconSrc = useMemo(() => {
    return `sage-app://${app.id}/icon.png`;
  }, [app.id]);

  const networkBadges = useMemo(() => {
    return [...(app.grantedPermissions.network ?? [])].sort((a, b) => {
      const aKey = `${a.scheme}://${a.host}`;
      const bKey = `${b.scheme}://${b.host}`;
      return aKey.localeCompare(bKey);
    });
  }, [app.grantedPermissions.network]);

  const hasPersistentStorage = app.grantedPermissions.persistentStorage;

  return (
    <Card>
      <CardHeader className='flex flex-row items-start justify-between gap-4 space-y-0'>
        <div className='min-w-0 space-y-2'>
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

          <div className='break-all text-xs text-muted-foreground'>
            Install dir: {app.installDir}
          </div>
        </div>

        <div className='flex shrink-0 items-center gap-2'>
          <Button onClick={() => navigate(`/apps/${app.id}`)}>Open</Button>
          <Button variant='outline' onClick={() => void onUninstall()}>
            <Trash2 className='mr-2 h-4 w-4' />
            Remove
          </Button>
        </div>
      </CardHeader>

      <CardContent className='space-y-3'>
        <div className='flex flex-wrap gap-2'>
          {hasPersistentStorage ? (
            <Badge variant='outline'>Persistent storage</Badge>
          ) : null}

          {networkBadges.map((entry) => (
            <Badge
              key={`${entry.scheme}://${entry.host}`}
              variant='outline'
              className='font-mono text-xs'
            >
              {entry.scheme}://{entry.host}
            </Badge>
          ))}

          {!hasPersistentStorage && networkBadges.length === 0 ? (
            <Badge variant='outline'>No permissions</Badge>
          ) : null}
        </div>
      </CardContent>
    </Card>
  );
}
