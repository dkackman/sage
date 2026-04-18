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

function isObject(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object';
}

function hasStringRunId(
  value: unknown,
): value is { runId: string } & Record<string, unknown> {
  return isObject(value) && typeof value.runId === 'string';
}

function isClearCycleReport(
  report: unknown,
): report is { type: 'clear_cycle'; data: SandboxStorageClearProbeResult } {
  return (
    isObject(report) &&
    report.type === 'clear_cycle' &&
    hasStringRunId(report.data)
  );
}

function isIsolationReport(
  report: unknown,
): report is { type: 'isolation'; data: SandboxIsolationProbeResult } {
  return (
    isObject(report) &&
    report.type === 'isolation' &&
    hasStringRunId(report.data)
  );
}

function isPersistenceWriteReport(report: unknown): report is {
  type: 'persistence_write';
  data: SandboxPersistenceWriteProbeResult;
} {
  return (
    isObject(report) &&
    report.type === 'persistence_write' &&
    hasStringRunId(report.data)
  );
}

function isPersistenceReadReport(report: unknown): report is {
  type: 'persistence_read';
  data: SandboxPersistenceReadProbeResult;
} {
  return (
    isObject(report) &&
    report.type === 'persistence_read' &&
    hasStringRunId(report.data)
  );
}

function isNetworkReport(
  report: unknown,
): report is { type: 'network'; data: SandboxNetworkProbeResult } {
  return (
    isObject(report) && report.type === 'network' && hasStringRunId(report.data)
  );
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

  if (isClearCycleReport(report)) {
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

  if (isIsolationReport(report)) {
    const run = getOrCreateRun(report.data.runId);
    run.isolation = replaceByAppId(run.isolation, {
      appId,
      data: report.data,
    });
    return true;
  }

  if (isPersistenceWriteReport(report)) {
    const run = getOrCreateRun(report.data.runId);
    run.persistenceWrite = replaceByAppId(run.persistenceWrite, {
      appId,
      data: report.data,
    });
    return true;
  }

  if (isPersistenceReadReport(report)) {
    const run = getOrCreateRun(report.data.runId);
    run.persistenceRead = replaceByAppId(run.persistenceRead, {
      appId,
      data: report.data,
    });
    return true;
  }

  if (isNetworkReport(report)) {
    const run = getOrCreateRun(report.data.runId);
    run.network = replaceByAppId(run.network, {
      appId,
      data: report.data,
    });
    return true;
  }

  return false;
}
