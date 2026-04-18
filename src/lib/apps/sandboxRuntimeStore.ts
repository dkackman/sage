import type { SageBridgeSendPayload } from '@/lib/apps/bridge';
import type {
  SandboxIsolationProbeResult,
  SandboxNetworkProbeResult,
  SandboxPersistenceReadProbeResult,
  SandboxPersistenceWriteProbeResult,
  SandboxStorageClearProbeResult,
} from '@/lib/apps/sandbox';

export interface SandboxAppResult<T> {
  appId: string;
  data: T;
}

export interface SandboxRunResults {
  isolation: SandboxAppResult<SandboxIsolationProbeResult>[];
  persistenceWrite: SandboxAppResult<SandboxPersistenceWriteProbeResult>[];
  persistenceRead: SandboxAppResult<SandboxPersistenceReadProbeResult>[];
  clearCycle: SandboxAppResult<SandboxStorageClearProbeResult>[];
  network: SandboxAppResult<SandboxNetworkProbeResult>[];
}

const runs = new Map<string, SandboxRunResults>();

function createEmptyRunResults(): SandboxRunResults {
  return {
    isolation: [],
    persistenceWrite: [],
    persistenceRead: [],
    clearCycle: [],
    network: [],
  };
}

function getOrCreateRun(runId: string): SandboxRunResults {
  let existing = runs.get(runId);
  if (!existing) {
    existing = createEmptyRunResults();
    runs.set(runId, existing);
  }
  return existing;
}

function replaceByAppId<T>(
  items: SandboxAppResult<T>[],
  next: SandboxAppResult<T>,
): SandboxAppResult<T>[] {
  const withoutSameApp = items.filter((item) => item.appId !== next.appId);
  return [...withoutSameApp, next];
}

export function resetSandboxRun(runId: string) {
  runs.set(runId, createEmptyRunResults());
}

export function clearAllSandboxRuns() {
  runs.clear();
}

export function getSandboxRunResults(runId: string): SandboxRunResults {
  return runs.get(runId) ?? createEmptyRunResults();
}

export function acceptSandboxBridgeSend(args: {
  appId: string;
  payload: SageBridgeSendPayload;
}): boolean {
  const { appId, payload } = args;

  if (payload.kind !== 'sandbox_report') {
    return false;
  }

  const report = payload.report;

  if (report.type === 'clear_cycle') {
    const run = getOrCreateRun(report.data.runId);
    run.clearCycle = replaceByAppId(run.clearCycle, {
      appId,
      data: report.data,
    });
    return true;
  }

  if (!appId.startsWith('__sage_test_')) {
    return false;
  }

  switch (report.type) {
    case 'isolation': {
      const run = getOrCreateRun(report.data.runId);
      run.isolation = replaceByAppId(run.isolation, {
        appId,
        data: report.data,
      });
      return true;
    }

    case 'persistence_write': {
      const run = getOrCreateRun(report.data.runId);
      run.persistenceWrite = replaceByAppId(run.persistenceWrite, {
        appId,
        data: report.data,
      });
      return true;
    }

    case 'persistence_read': {
      const run = getOrCreateRun(report.data.runId);
      run.persistenceRead = replaceByAppId(run.persistenceRead, {
        appId,
        data: report.data,
      });
      return true;
    }

    case 'network': {
      const run = getOrCreateRun(report.data.runId);
      run.network = replaceByAppId(run.network, {
        appId,
        data: report.data,
      });
      return true;
    }
  }
}
