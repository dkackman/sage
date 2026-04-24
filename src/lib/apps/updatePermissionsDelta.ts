import type {
  UserSageApp,
  SageAppPackageManifest,
  SageAppUrlPreview,
  SageGrantedPermissions,
  SageNetworkPermissionTarget,
  SageRequestedPermissions,
  UserBridgeCapability,
} from '@/bindings';
import {
  cloneNetwork,
  networkKey,
  sortCapabilities,
  sortNetwork,
} from '@/lib/apps/permissionCollections';

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
    required: UserBridgeCapability[];
    optional: UserBridgeCapability[];
  };
  addedRequestedNetwork: {
    required: SageNetworkPermissionTarget[];
    optional: SageNetworkPermissionTarget[];
  };

  requiredCapabilitiesToGrant: UserBridgeCapability[];
  requiredNetworkToGrant: SageNetworkPermissionTarget[];

  removedGrantedCapabilities: UserBridgeCapability[];
  removedGrantedNetwork: SageNetworkPermissionTarget[];

  nextGrantedPermissions: SageGrantedPermissions;
  requiresUserReview: boolean;
}

export function getAppUpdatePermissionsDelta(
  app: UserSageApp,
  preview: SageAppUrlPreview | SageAppPackageManifest,
): AppUpdatePermissionsDelta {
  const previousRequested = app.common.requestedPermissions;
  const nextRequested = getManifestRequestedPermissions(preview);

  const previousCaps = getRequestedCapabilities(previousRequested);
  const nextCaps = getRequestedCapabilities(nextRequested);

  const previousNetwork = getRequestedNetwork(previousRequested);
  const nextNetwork = getRequestedNetwork(nextRequested);

  const oldGrantedCapabilities =
    app.common.grantedPermissions.capabilities ?? [];
  const oldGrantedNetwork =
    app.common.grantedPermissions.network.whitelist ?? [];

  const oldGrantedCapabilitiesSet = new Set(oldGrantedCapabilities);
  const oldGrantedNetworkSet = new Set(oldGrantedNetwork.map(networkKey));

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

  const requiredCapabilitiesToGrant = sortCapabilities(
    nextCaps.required.filter((key) => !oldGrantedCapabilitiesSet.has(key)),
  );

  const requiredNetworkToGrant = sortNetwork(
    nextNetwork.required.filter(
      (entry) => !oldGrantedNetworkSet.has(networkKey(entry)),
    ),
  );

  const nextAllowedCapsSet = new Set([
    ...nextCaps.required,
    ...nextCaps.optional,
  ]);
  const nextAllowedNetworkSet = new Set([
    ...nextNetwork.required.map(networkKey),
    ...nextNetwork.optional.map(networkKey),
  ]);

  const removedGrantedCapabilities = sortCapabilities(
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

  const nextGrantedCapabilities = sortCapabilities(
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

  const requiresUserReview =
    requiredCapabilitiesToGrant.length > 0 || requiredNetworkToGrant.length > 0;

  return {
    addedRequestedCapabilities,
    addedRequestedNetwork,
    requiredCapabilitiesToGrant,
    requiredNetworkToGrant,
    removedGrantedCapabilities,
    removedGrantedNetwork,
    nextGrantedPermissions,
    requiresUserReview,
  };
}
