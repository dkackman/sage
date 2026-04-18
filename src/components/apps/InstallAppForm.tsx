import { useState } from 'react';
import type {
  SageAppPackageManifest,
  SageAppUrlPreview,
  SageNetworkPermissionTarget,
} from '@/bindings';
import { formatAppError } from '@/lib/apps/formatAppError.ts';
import { InstallSourceCard } from './InstallSourceCard';
import {
  buildFullyForbiddenPermissions,
  buildInitialGrantedNetworkWhitelist,
  buildInitialGrantedPermissions,
} from '@/components/apps/permissions/permissionUtils.ts';
import { InstallPermissionsDialog } from '@/components/apps/permissions/InstallDialog.tsx';

interface Props {
  onPreviewZip: (zipPath: string) => Promise<SageAppPackageManifest>;
  onPreviewUrl: (appUrl: string) => Promise<SageAppUrlPreview>;
  onInstallZip: (
    zipPath: string,
    permissions: string[],
    networkWhitelist: SageNetworkPermissionTarget[],
  ) => Promise<void>;
  onInstallUrl: (
    appUrl: string,
    permissions: string[],
    networkWhitelist: SageNetworkPermissionTarget[],
  ) => Promise<void>;
}

type InstallSource =
  | {
      kind: 'zip';
      zipPath: string;
      manifest: SageAppPackageManifest;
    }
  | {
      kind: 'url';
      appUrl: string;
      preview: SageAppUrlPreview;
    };

export function InstallAppForm({
  onPreviewZip,
  onPreviewUrl,
  onInstallZip,
  onInstallUrl,
}: Props) {
  const [installing, setInstalling] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [urlInput, setUrlInput] = useState('');
  const [source, setSource] = useState<InstallSource | null>(null);
  const [grantedPermissions, setGrantedPermissions] = useState<string[]>(
    buildFullyForbiddenPermissions(),
  );
  const [grantedNetworkWhitelist, setGrantedNetworkWhitelist] = useState<
    SageNetworkPermissionTarget[]
  >([]);

  async function handleSelectZipPath(zipPath: string) {
    try {
      setError(null);

      const nextManifest = await onPreviewZip(zipPath);

      setSource({
        kind: 'zip',
        zipPath,
        manifest: nextManifest,
      });

      setGrantedPermissions(buildInitialGrantedPermissions(nextManifest));
      setGrantedNetworkWhitelist(
        buildInitialGrantedNetworkWhitelist(nextManifest),
      );
    } catch (err) {
      setError(formatAppError(err));
    }
  }

  async function handlePreviewUrl() {
    try {
      setError(null);

      const preview = await onPreviewUrl(urlInput.trim());

      setSource({
        kind: 'url',
        appUrl: preview.appUrl,
        preview,
      });

      setGrantedPermissions(buildInitialGrantedPermissions(preview.manifest));
      setGrantedNetworkWhitelist(
        buildInitialGrantedNetworkWhitelist(preview.manifest),
      );
    } catch (err) {
      setError(formatAppError(err));
    }
  }

  async function confirmInstall() {
    if (!source) {
      return;
    }

    try {
      setInstalling(true);
      setError(null);

      if (source.kind === 'zip') {
        await onInstallZip(
          source.zipPath,
          grantedPermissions,
          grantedNetworkWhitelist,
        );
      } else {
        await onInstallUrl(
          source.appUrl,
          grantedPermissions,
          grantedNetworkWhitelist,
        );
      }

      setSource(null);
      setGrantedPermissions(buildFullyForbiddenPermissions());
      setGrantedNetworkWhitelist([]);
      setUrlInput('');
    } catch (err) {
      setError(formatAppError(err));
    } finally {
      setInstalling(false);
    }
  }

  function resetDialog() {
    setSource(null);
    setGrantedPermissions(buildFullyForbiddenPermissions());
    setGrantedNetworkWhitelist([]);
    setError(null);
  }

  return (
    <>
      <InstallSourceCard
        installing={installing}
        urlInput={urlInput}
        onUrlInputChange={setUrlInput}
        onSelectZipPath={handleSelectZipPath}
        onPreviewUrl={handlePreviewUrl}
        error={error}
      />

      <InstallPermissionsDialog
        source={source}
        error={error}
        installing={installing}
        grantedPermissions={grantedPermissions}
        grantedNetworkWhitelist={grantedNetworkWhitelist}
        onGrantedPermissionsChange={setGrantedPermissions}
        onGrantedNetworkWhitelistChange={setGrantedNetworkWhitelist}
        onCancel={resetDialog}
        onConfirm={confirmInstall}
      />
    </>
  );
}
