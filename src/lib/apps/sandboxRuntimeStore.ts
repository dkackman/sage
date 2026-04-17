import type { SageBridgeSendPayload } from '@/lib/apps/bridge';
import type {
  SandboxIsolationProbeResult,
  SandboxNetworkProbeResult,
  SandboxPersistenceReadProbeResult,
  SandboxPersistenceWriteProbeResult,
} from '@/lib/apps/sandbox';
export interface SandboxRunResults {
  isolation: SandboxIsolationProbeResult[];
  persistenceWrite: SandboxPersistenceWriteProbeResult[];
  persistenceRead: SandboxPersistenceReadProbeResult[];
  network: SandboxNetworkProbeResult[];
}

const runs = new Map<string, SandboxRunResults>();

function createEmptyRunResults(): SandboxRunResults {
  return {
    isolation: [],
    persistenceWrite: [],
    persistenceRead: [],
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

function replaceByMode<T extends { mode: string }>(items: T[], next: T): T[] {
  const withoutSameMode = items.filter((item) => item.mode !== next.mode);
  return [...withoutSameMode, next];
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

  if (!appId.startsWith('__sage_test_')) {
    return false;
  }

  const report = payload.report;

  switch (report.type) {
    case 'isolation': {
      const run = getOrCreateRun(report.data.runId);
      run.isolation = replaceByMode(run.isolation, report.data);
      return true;
    }

    case 'persistence_write': {
      const run = getOrCreateRun(report.data.runId);
      run.persistenceWrite = replaceByMode(run.persistenceWrite, report.data);
      return true;
    }

    case 'persistence_read': {
      const run = getOrCreateRun(report.data.runId);
      run.persistenceRead = replaceByMode(run.persistenceRead, report.data);
      return true;
    }

    case 'network': {
      const run = getOrCreateRun(report.data.runId);
      run.network = replaceByMode(run.network, report.data);
      return true;
    }
  }
}
