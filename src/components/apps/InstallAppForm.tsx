import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { SageAppManifest, SageAppPermission } from '@/lib/apps/types';
import { useState } from 'react';

interface Props {
  onInstall: (manifest: SageAppManifest) => void;
}

const DEFAULT_PERMISSIONS: SageAppPermission[] = [
  'wallet.read.addresses',
  'wallet.read.balance',
  'wallet.tx.create',
  'wallet.tx.submit',
  'storage.readwrite',
];

export function InstallAppForm({ onInstall }: Props) {
  const [id, setId] = useState('chia.vanity');
  const [name, setName] = useState('Vanity Address Generator');
  const [version, setVersion] = useState('0.1.0');
  const [description, setDescription] = useState(
    'External wallet app installed manually for development.',
  );
  const [entry, setEntry] = useState('http://localhost:1421');

  function handleInstall() {
    const manifest: SageAppManifest = {
      id: id.trim(),
      name: name.trim(),
      version: version.trim(),
      description: description.trim(),
      entry: entry.trim(),
      permissions: DEFAULT_PERMISSIONS,
      verified: false,
      publisher: 'manual',
      source: 'manual',
      installDir: null,
      icon: null,
    };

    if (!manifest.id || !manifest.name || !manifest.entry) {
      return;
    }

    onInstall(manifest);
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>Install App</CardTitle>
      </CardHeader>
      <CardContent className='space-y-4'>
        <Input
          value={id}
          onChange={(e) => setId(e.target.value)}
          placeholder='App ID'
        />
        <Input
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder='App name'
        />
        <Input
          value={version}
          onChange={(e) => setVersion(e.target.value)}
          placeholder='Version'
        />
        <Input
          value={entry}
          onChange={(e) => setEntry(e.target.value)}
          placeholder='Entry URL or installed file path'
        />
        <Input
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          placeholder='Description'
        />
        <Button onClick={handleInstall}>Install / Update</Button>
      </CardContent>
    </Card>
  );
}

