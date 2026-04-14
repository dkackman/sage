import { InstalledAppsState, SageAppManifest } from '@/lib/apps/types';
import { useMemo } from 'react';
import { useLocalStorage } from 'usehooks-ts';

const INSTALLED_APPS_STORAGE_KEY = 'sage-wallet-installed-apps';

const DEFAULT_INSTALLED_APPS: InstalledAppsState = {
  'chia.vanity': {
    id: 'chia.vanity',
    name: 'Vanity Address Generator',
    version: '0.1.0',
    description:
      'Brute-force Chia receive address prefixes and later sweep matching derivation indexes.',
    entry: 'http://localhost:1421',
    permissions: [
      'wallet.read.addresses',
      'wallet.read.balance',
      'wallet.tx.create',
      'wallet.tx.submit',
      'storage.readwrite',
    ],
    verified: false,
    publisher: 'manual',
    source: 'manual',
    installDir: null,
    icon: null,
  },
};

export function useApps() {
  const [installedApps, setInstalledApps] = useLocalStorage<InstalledAppsState>(
    INSTALLED_APPS_STORAGE_KEY,
    DEFAULT_INSTALLED_APPS,
  );

  const apps = useMemo(() => {
    return Object.values(installedApps).sort((a, b) =>
      a.name.localeCompare(b.name),
    );
  }, [installedApps]);

  function installApp(manifest: SageAppManifest) {
    setInstalledApps((prev) => ({
      ...prev,
      [manifest.id]: manifest,
    }));
  }

  function uninstallApp(appId: string) {
    setInstalledApps((prev) => {
      const next = { ...prev };
      delete next[appId];
      return next;
    });
  }

  function isInstalled(appId: string) {
    return !!installedApps[appId];
  }

  function getApp(appId: string): SageAppManifest | undefined {
    return installedApps[appId];
  }

  return {
    apps,
    installApp,
    uninstallApp,
    isInstalled,
    getApp,
  };
}

