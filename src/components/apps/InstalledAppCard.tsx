import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { SageAppManifest } from '@/lib/apps/types';
import { ShieldCheck, Trash2 } from 'lucide-react';
import { useNavigate } from 'react-router-dom';

interface Props {
  manifest: SageAppManifest;
  onUninstall: () => void;
}

function permissionLabel(permission: string) {
  switch (permission) {
    case 'wallet.read.addresses':
      return 'Read addresses';
    case 'wallet.read.balance':
      return 'Read balances';
    case 'wallet.tx.create':
      return 'Create transactions';
    case 'wallet.tx.submit':
      return 'Submit transactions';
    case 'storage.readwrite':
      return 'Store app data';
    default:
      return permission;
  }
}

export function InstalledAppCard({ manifest, onUninstall }: Props) {
  const navigate = useNavigate();

  return (
    <Card>
      <CardHeader className='flex flex-row items-start justify-between space-y-0 gap-4'>
        <div className='space-y-2'>
          <CardTitle className='flex items-center gap-2'>
            <span>{manifest.name}</span>
            {manifest.verified && (
              <Badge variant='secondary' className='gap-1'>
                <ShieldCheck className='h-3.5 w-3.5' />
                Verified
              </Badge>
            )}
            {manifest.source && (
              <Badge variant='outline'>{manifest.source}</Badge>
            )}
          </CardTitle>
          <div className='text-sm text-muted-foreground'>
            v{manifest.version}
            {manifest.publisher ? ` • ${manifest.publisher}` : ''}
          </div>
          <p className='text-sm text-muted-foreground max-w-2xl'>
            {manifest.description}
          </p>
          <div className='text-xs text-muted-foreground break-all'>
            Entry: {manifest.entry}
          </div>
        </div>

        <div className='flex items-center gap-2 shrink-0'>
          <Button onClick={() => navigate(`/apps/${manifest.id}`)}>Open</Button>
          <Button variant='outline' onClick={onUninstall}>
            <Trash2 className='h-4 w-4 mr-2' />
            Remove
          </Button>
        </div>
      </CardHeader>

      <CardContent className='space-y-3'>
        <div className='flex flex-wrap gap-2'>
          {manifest.permissions.map((permission) => (
            <Badge key={permission} variant='outline'>
              {permissionLabel(permission)}
            </Badge>
          ))}
        </div>
      </CardContent>
    </Card>
  );
}

