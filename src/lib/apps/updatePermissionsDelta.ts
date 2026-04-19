import type {
  InstalledSageApp,
  SageAppPackageManifest,
  SageAppUrlPreview,
  SageGrantedPermissions,
  SageNetworkPermissionTarget,
  SageRequestedPermissions,
} from '@/bindings';

function networkKey(entry: SageNetworkPermissionTarget): string {
  return `${entry.scheme}://${entry.host}`;
}

function sortStrings(values: Iterable<string>): string[] {
  return [...values].sort((a, b) => a.localeCompare(b));
}

function sortNetwork(
  values: Iterable<SageNetworkPermissionTarget>,
): SageNetworkPermissionTarget[] {
  return [...values].sort((a, b) => networkKey(a).localeCompare(networkKey(b)));
}

function cloneNetwork(
  values: Iterable<SageNetworkPermissionTarget>,
): SageNetworkPermissionTarget[] {
  return [...values].map((entry) => ({
    scheme: entry.scheme,
    host: entry.host,
  }));
}

function getRequestedCapabilities(permissions: SageRequestedPermissions) {
  return {
    required: permissions.capabilities.required ?? [],
    optional: permissions.capabilities.optional ?? [],
  };
}

function getRequestedNetwork(permissions: SageRequestedPermissions) {
  return {
    required: permissions.network.whitelist.required ?? [],
    optional: permissions.network.whitelist.optional ?? [],
  };
}

function getManifestRequestedPermissions(
  manifest: SageAppPackageManifest | SageAppUrlPreview,
): SageRequestedPermissions {
  return 'manifest' in manifest
    ? manifest.manifest.permissions
    : manifest.permissions;
}

export interface AppUpdatePermissionsDelta {
  addedRequestedCapabilities: {
    required: string[];
    optional: string[];
  };
  addedRequestedNetwork: {
    required: SageNetworkPermissionTarget[];
    optional: SageNetworkPermissionTarget[];
  };
  removedGrantedCapabilities: string[];
  removedGrantedNetwork: SageNetworkPermissionTarget[];
  nextGrantedPermissions: SageGrantedPermissions;
  requiresUserReview: boolean;
}

export function getAppUpdatePermissionsDelta(
  app: InstalledSageApp,
  preview: SageAppUrlPreview | SageAppPackageManifest,
): AppUpdatePermissionsDelta {
  const previousRequested = app.requestedPermissions;
  const nextRequested = getManifestRequestedPermissions(preview);

  const previousCaps = getRequestedCapabilities(previousRequested);
  const nextCaps = getRequestedCapabilities(nextRequested);

  const previousNetwork = getRequestedNetwork(previousRequested);
  const nextNetwork = getRequestedNetwork(nextRequested);

  const oldGrantedCapabilities = app.grantedPermissions.capabilities ?? [];
  const oldGrantedNetwork = app.grantedPermissions.network.whitelist ?? [];

  const previousRequestedCapsSet = new Set([
    ...previousCaps.required,
    ...previousCaps.optional,
  ]);

  const previousRequestedNetworkSet = new Set([
    ...previousNetwork.required.map(networkKey),
    ...previousNetwork.optional.map(networkKey),
  ]);

  const addedRequestedCapabilities = {
    required: nextCaps.required.filter(
      (key) => !previousRequestedCapsSet.has(key),
    ),
    optional: nextCaps.optional.filter(
      (key) => !previousRequestedCapsSet.has(key),
    ),
  };

  const addedRequestedNetwork = {
    required: cloneNetwork(
      nextNetwork.required.filter(
        (entry) => !previousRequestedNetworkSet.has(networkKey(entry)),
      ),
    ),
    optional: cloneNetwork(
      nextNetwork.optional.filter(
        (entry) => !previousRequestedNetworkSet.has(networkKey(entry)),
      ),
    ),
  };

  const nextAllowedCapsSet = new Set([
    ...nextCaps.required,
    ...nextCaps.optional,
  ]);
  const nextAllowedNetworkSet = new Set([
    ...nextNetwork.required.map(networkKey),
    ...nextNetwork.optional.map(networkKey),
  ]);

  const removedGrantedCapabilities = sortStrings(
    oldGrantedCapabilities.filter((key) => !nextAllowedCapsSet.has(key)),
  );

  const removedGrantedNetwork = sortNetwork(
    oldGrantedNetwork.filter(
      (entry) => !nextAllowedNetworkSet.has(networkKey(entry)),
    ),
  );

  const retainedGrantedCapabilities = oldGrantedCapabilities.filter((key) =>
    nextAllowedCapsSet.has(key),
  );

  const nextGrantedCapabilities = sortStrings(
    new Set([...retainedGrantedCapabilities, ...nextCaps.required]),
  );

  const retainedGrantedNetwork = oldGrantedNetwork.filter((entry) =>
    nextAllowedNetworkSet.has(networkKey(entry)),
  );

  const nextGrantedNetworkMap = new Map<string, SageNetworkPermissionTarget>();

  for (const entry of retainedGrantedNetwork) {
    nextGrantedNetworkMap.set(networkKey(entry), {
      scheme: entry.scheme,
      host: entry.host,
    });
  }

  for (const entry of nextNetwork.required) {
    nextGrantedNetworkMap.set(networkKey(entry), {
      scheme: entry.scheme,
      host: entry.host,
    });
  }

  const nextGrantedPermissions: SageGrantedPermissions = {
    capabilities: nextGrantedCapabilities,
    network: {
      whitelist: sortNetwork(nextGrantedNetworkMap.values()),
    },
  };

  const oldGrantedCapabilitiesSet = new Set(oldGrantedCapabilities);
  const oldGrantedNetworkSet = new Set(oldGrantedNetwork.map(networkKey));

  const grantedCapabilitiesExpanded = nextGrantedCapabilities.some(
    (key) => !oldGrantedCapabilitiesSet.has(key),
  );

  const nextGrantedNetwork = nextGrantedPermissions.network.whitelist;
  const grantedNetworkExpanded = nextGrantedNetwork.some(
    (entry) => !oldGrantedNetworkSet.has(networkKey(entry)),
  );

  const requiresUserReview =
    grantedCapabilitiesExpanded || grantedNetworkExpanded;

  return {
    addedRequestedCapabilities,
    addedRequestedNetwork,
    removedGrantedCapabilities,
    removedGrantedNetwork,
    nextGrantedPermissions,
    requiresUserReview,
  };
}
