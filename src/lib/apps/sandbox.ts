import type { InstalledSageApp } from '@/bindings';

export type SandboxCapability =
  | 'storage_isolation_from_sage'
  | 'storage_persistence_normal'
  | 'storage_non_persistence_incognito'
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
  mode: string;
  persistentStorage: boolean;
  localStorageVisible: boolean;
  cookieVisible: boolean;
  indexedDbVisible: boolean;
  error: string | null;
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
      storage_persistence_normal: makeCapabilityResult('pending'),
      storage_non_persistence_incognito: makeCapabilityResult('pending'),
      network_allowlist_enforced: makeCapabilityResult('pending'),
    },
  };
}

export function buildIsolationOnlySandboxState(
  passed: boolean,
  details: string | null,
): SandboxState {
  const checkedAt = Date.now();

  return {
    overallCriticalStatus: passed ? 'passed' : 'failed',
    startedAt: checkedAt,
    finishedAt: checkedAt,
    capabilities: {
      storage_isolation_from_sage: {
        status: passed ? 'passed' : 'failed',
        checkedAt,
        details,
      },
      storage_persistence_normal: {
        status: 'pending',
        checkedAt: null,
        details: 'Not implemented yet.',
      },
      storage_non_persistence_incognito: {
        status: 'pending',
        checkedAt: null,
        details: 'Not implemented yet.',
      },
      network_allowlist_enforced: {
        status: 'pending',
        checkedAt: null,
        details: 'Not implemented yet.',
      },
    },
  };
}

export function getRequiredSandboxCapabilities(
  _app: InstalledSageApp,
): SandboxCapability[] {
  return ['storage_isolation_from_sage'];
}

export function evaluateAppLaunchGate(
  app: InstalledSageApp,
  sandbox: SandboxState,
): AppLaunchGateResult {
  if (
    sandbox.overallCriticalStatus === 'pending' ||
    sandbox.overallCriticalStatus === 'running'
  ) {
    return {
      allowed: false,
      kind: 'running',
      capability: null,
      message: 'Sandbox tests are still running.',
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
    case 'network_allowlist_enforced':
      return 'network allowlist enforcement';
  }
}
