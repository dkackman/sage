import type { InstalledSageApp } from '@/bindings';

export type SandboxCapability =
  | 'storage_isolation_from_sage'
  | 'storage_persistence_normal'
  | 'storage_non_persistence_incognito'
  | 'storage_clear_cycle'
  | 'network_allowlist_enforced';

export type SandboxCapabilityStatus =
  | 'pending'
  | 'running'
  | 'passed'
  | 'failed';

export interface SandboxCapabilityResult {
  status: SandboxCapabilityStatus;
  checkedAt: number | null;
  details: string | null;
}

export interface SandboxState {
  overallCriticalStatus: SandboxCapabilityStatus;
  capabilities: Record<SandboxCapability, SandboxCapabilityResult>;
  startedAt: number | null;
  finishedAt: number | null;
}

export interface AppLaunchGateResult {
  allowed: boolean;
  kind: 'allowed' | 'running' | 'failed';
  capability: SandboxCapability | null;
  message: string | null;
}

export interface SandboxIsolationProbeResult {
  runId: string;
  localStorageVisible: boolean;
  indexedDbVisible: boolean;
  error: string | null;
}

export interface SandboxPersistenceWriteProbeResult {
  runId: string;
  localStorageWrote: boolean;
  indexedDbWrote: boolean;
  error: string | null;
}

export interface SandboxPersistenceReadProbeResult {
  runId: string;
  localStoragePresent: boolean;
  indexedDbPresent: boolean;
  error: string | null;
}

export interface SandboxNetworkProbeResult {
  runId: string;
  allowedUrl: string;
  blockedUrl: string;
  allowedOk: boolean;
  blockedOk: boolean;
  error: string | null;
}

export type SandboxStorageClearProbePhase =
  | 'write'
  | 'check_present'
  | 'check_absent';

export interface SandboxStorageClearProbeResult {
  runId: string;
  phase: SandboxStorageClearProbePhase;
  localStoragePresent: boolean;
  indexedDbPresent: boolean;
  error: string | null;
}

let storageClearCapabilityPassed = false;

export function setStorageClearCapabilityPassed(value: boolean) {
  storageClearCapabilityPassed = value;
}

export function isStorageClearCapabilityPassed() {
  return storageClearCapabilityPassed;
}

function makeCapabilityResult(
  status: SandboxCapabilityStatus,
  details: string | null = null,
): SandboxCapabilityResult {
  return {
    status,
    checkedAt: null,
    details,
  };
}

export function buildInitialSandboxState(): SandboxState {
  return {
    overallCriticalStatus: 'pending',
    startedAt: null,
    finishedAt: null,
    capabilities: {
      storage_isolation_from_sage: makeCapabilityResult('pending'),
      storage_persistence_normal: makeCapabilityResult('pending'),
      storage_non_persistence_incognito: makeCapabilityResult('pending'),
      storage_clear_cycle: makeCapabilityResult('pending'),
      network_allowlist_enforced: makeCapabilityResult('pending'),
    },
  };
}

export function buildRunningSandboxState(): SandboxState {
  return {
    overallCriticalStatus: 'running',
    startedAt: Date.now(),
    finishedAt: null,
    capabilities: {
      storage_isolation_from_sage: makeCapabilityResult('running'),
      storage_persistence_normal: makeCapabilityResult('running'),
      storage_non_persistence_incognito: makeCapabilityResult('running'),
      storage_clear_cycle: makeCapabilityResult('running'),
      network_allowlist_enforced: makeCapabilityResult('running'),
    },
  };
}

export function buildCompletedSandboxState(args: {
  isolation: { passed: boolean; details: string | null };
  persistenceNormal: { passed: boolean; details: string | null };
  persistenceIncognito: { passed: boolean; details: string | null };
  clearCycle: { passed: boolean; details: string | null };
  network: { passed: boolean; details: string | null };
}): SandboxState {
  const checkedAt = Date.now();

  return {
    overallCriticalStatus: args.isolation.passed ? 'passed' : 'failed',
    startedAt: checkedAt,
    finishedAt: checkedAt,
    capabilities: {
      storage_isolation_from_sage: {
        status: args.isolation.passed ? 'passed' : 'failed',
        checkedAt,
        details: args.isolation.details,
      },
      storage_persistence_normal: {
        status: args.persistenceNormal.passed ? 'passed' : 'failed',
        checkedAt,
        details: args.persistenceNormal.details,
      },
      storage_non_persistence_incognito: {
        status: args.persistenceIncognito.passed ? 'passed' : 'failed',
        checkedAt,
        details: args.persistenceIncognito.details,
      },
      storage_clear_cycle: {
        status: args.clearCycle.passed ? 'passed' : 'failed',
        checkedAt,
        details: args.clearCycle.details,
      },
      network_allowlist_enforced: {
        status: args.network.passed ? 'passed' : 'failed',
        checkedAt,
        details: args.network.details,
      },
    },
  };
}

export function getRequiredSandboxCapabilities(
  app: InstalledSageApp,
): SandboxCapability[] {
  const required: SandboxCapability[] = ['storage_isolation_from_sage'];

  if (app.grantedPermissions.includes('persistent_storage')) {
    required.push('storage_persistence_normal');
  } else {
    required.push('storage_non_persistence_incognito');
  }

  if ((app.activeSnapshot.manifest.network?.whitelist?.length ?? 0) > 0) {
    required.push('network_allowlist_enforced');
  }

  return required;
}

export function evaluateAppLaunchGate(
  app: InstalledSageApp,
  sandbox: SandboxState,
): AppLaunchGateResult {
  const isolation = sandbox.capabilities.storage_isolation_from_sage;

  if (isolation.status === 'pending' || isolation.status === 'running') {
    return {
      allowed: false,
      kind: 'running',
      capability: 'storage_isolation_from_sage',
      message: `Sandbox tests are still running for ${formatCapabilityLabel(
        'storage_isolation_from_sage',
      )}.`,
    };
  }

  if (isolation.status === 'failed') {
    return {
      allowed: false,
      kind: 'failed',
      capability: 'storage_isolation_from_sage',
      message:
        isolation.details ??
        `Sandbox test failed for ${formatCapabilityLabel(
          'storage_isolation_from_sage',
        )}.`,
    };
  }

  const required = getRequiredSandboxCapabilities(app);

  for (const capability of required) {
    const result = sandbox.capabilities[capability];

    if (result.status === 'pending' || result.status === 'running') {
      return {
        allowed: false,
        kind: 'running',
        capability,
        message: `Sandbox tests are still running for ${formatCapabilityLabel(capability)}.`,
      };
    }

    if (result.status === 'failed') {
      return {
        allowed: false,
        kind: 'failed',
        capability,
        message:
          result.details ??
          `Sandbox test failed for ${formatCapabilityLabel(capability)}.`,
      };
    }
  }

  return {
    allowed: true,
    kind: 'allowed',
    capability: null,
    message: null,
  };
}

export function formatCapabilityLabel(capability: SandboxCapability): string {
  switch (capability) {
    case 'storage_isolation_from_sage':
      return 'storage isolation from Sage';
    case 'storage_persistence_normal':
      return 'persistent storage behavior';
    case 'storage_non_persistence_incognito':
      return 'incognito storage behavior';
    case 'storage_clear_cycle':
      return 'storage clear cycle behavior';
    case 'network_allowlist_enforced':
      return 'network allowlist enforcement';
  }
}
