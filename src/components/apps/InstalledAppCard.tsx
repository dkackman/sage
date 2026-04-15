import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import type { InstalledSageApp, SageAppUrlPreview } from '@/bindings';
import { Trash2 } from 'lucide-react';
import { useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useAppRuntimePresence } from '@/hooks/useAppRuntimePresence';

interface Props {
  app: InstalledSageApp;
  updatePreview: SageAppUrlPreview | null | undefined;
  busy: boolean;
  onUninstall: () => Promise<void>;
  onCheckForUpdate: () => Promise<void>;
  onDownloadUpdate: () => Promise<void>;
  onApplyUpdate: () => Promise<void>;
}

export function InstalledAppCard({
  app,
  updatePreview,
  busy,
  onUninstall,
  onCheckForUpdate,
  onDownloadUpdate,
  onApplyUpdate,
}: Props) {
  const navigate = useNavigate();
  const isRunning = useAppRuntimePresence(app.id);
  const [removing, setRemoving] = useState(false);

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
  const isUrlApp = app.source?.kind === 'url';
  const hasPendingUpdate = !!app.pendingUpdate;
  const showUpdateButton = hasPendingUpdate || !!updatePreview;
  const updateLabel = isRunning ? 'Update and reopen' : 'Update';

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
            {isRunning ? <Badge variant='outline'>running</Badge> : null}
            {hasPendingUpdate ? (
              <Badge variant='outline'>update ready</Badge>
            ) : null}
            {!hasPendingUpdate && updatePreview ? (
              <Badge variant='outline'>update available</Badge>
            ) : null}
          </CardTitle>

          <div className='text-sm text-muted-foreground'>v{app.version}</div>

          <div className='break-all text-xs text-muted-foreground'>
            Install dir: {app.installDir}
          </div>

          {isUrlApp && updatePreview ? (
            <div className='text-xs text-amber-600'>
              New version available: v{updatePreview.manifest.version}
            </div>
          ) : null}

          {isUrlApp && hasPendingUpdate ? (
            <div className='text-xs text-amber-600'>
              Downloaded update ready: v{app.pendingUpdate?.manifest.version}
            </div>
          ) : null}
        </div>

        <div className='flex shrink-0 items-center gap-2'>
          <Button onClick={() => navigate(`/apps/${app.id}`)}>Open</Button>

          {isUrlApp ? (
            <>
              {showUpdateButton ? (
                <Button
                  variant='outline'
                  disabled={busy}
                  onClick={() => void onApplyUpdate()}
                >
                  {busy ? 'Working...' : updateLabel}
                </Button>
              ) : (
                <Button
                  variant='outline'
                  disabled={busy}
                  onClick={() =>
                    void (async () => {
                      const preview = await onCheckForUpdate();
                      if (preview == null) {
                        return;
                      }
                      await onDownloadUpdate();
                    })()
                  }
                >
                  {busy ? 'Checking...' : 'Check for update'}
                </Button>
              )}
            </>
          ) : null}

          <Button
            variant='outline'
            disabled={removing || busy}
            onClick={() =>
              void (async () => {
                try {
                  setRemoving(true);
                  await onUninstall();
                } finally {
                  setRemoving(false);
                }
              })()
            }
          >
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
