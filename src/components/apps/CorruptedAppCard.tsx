import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { CorruptedInstalledSageApp } from '@/bindings';
import { Trash2, TriangleAlert } from 'lucide-react';

interface Props {
  app: CorruptedInstalledSageApp;
  onRemove: () => Promise<void>;
}

export function CorruptedAppCard({ app, onRemove }: Props) {
  return (
    <Card>
      <CardHeader className='flex flex-row items-start justify-between space-y-0 gap-4'>
        <div className='space-y-2 min-w-0'>
          <CardTitle className='flex items-center gap-3'>
            <TriangleAlert className='h-5 w-5 text-destructive' />
            <span>{app.id}</span>
            <Badge variant='destructive'>corrupted</Badge>
          </CardTitle>

          <div className='text-xs text-muted-foreground break-all'>
            Install dir: {app.install_dir}
          </div>
        </div>

        <div className='flex items-center gap-2 shrink-0'>
          <Button variant='outline' onClick={() => void onRemove()}>
            <Trash2 className='h-4 w-4 mr-2' />
            Remove
          </Button>
        </div>
      </CardHeader>

      <CardContent>
        <div className='text-sm text-destructive break-words'>{app.error}</div>
      </CardContent>
    </Card>
  );
}

