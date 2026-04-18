import type { InstalledSageApp } from '@/bindings';
import type { SandboxStorageClearProbePhase } from '@/lib/apps/sandbox';
import {
  clearAppRuntimeBrowsingData,
  closeAppRuntime,
  startInternalInlineRuntime,
} from '@/lib/apps/runtimeRegistry';
import {
  getSandboxRunResults,
  resetSandboxRun,
} from '@/lib/apps/sandboxRuntimeStore';

const STORAGE_CLEAR_PROBE_PATH =
  '/__sage/runtime-apps/storage-clear-probe/index.html';

function uniqueRunId(prefix: string): string {
  return `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

async function pollPhase(
  appId: string,
  runId: string,
  phase: SandboxStorageClearProbePhase,
  timeoutMs = 10000,
) {
  const startedAt = Date.now();

  for (;;) {
    const results = getSandboxRunResults(runId).clearCycle;
    const match = results.find(
      (item) => item.appId === appId && item.data.phase === phase,
    );

    if (match) {
      return match.data;
    }

    if (Date.now() - startedAt >= timeoutMs) {
      throw new Error(`Timed out waiting for storage clear phase ${phase}.`);
    }

    await new Promise((resolve) => window.setTimeout(resolve, 100));
  }
}

async function runProbePhase(
  app: InstalledSageApp,
  runId: string,
  phase: SandboxStorageClearProbePhase,
) {
  await startInternalInlineRuntime(app, {
    path: STORAGE_CLEAR_PROBE_PATH,
    query: {
      runId,
      phase,
    },
  });

  const result = await pollPhase(app.id, runId, phase);
  await closeAppRuntime(app.id, { timeoutMs: 8000 });
  return result;
}

export async function runStorageClearCycle(app: InstalledSageApp): Promise<{
  passed: boolean;
  details: string | null;
}> {
  const runId = uniqueRunId('storage-clear-cycle');
  resetSandboxRun(runId);

  await closeAppRuntime(app.id, { timeoutMs: 8000 }).catch(() => {
    //
  });

  try {
    const writeResult = await runProbePhase(app, runId, 'write');
    if (writeResult.error) {
      return {
        passed: false,
        details: `Storage clear write probe failed: ${writeResult.error}`,
      };
    }

    const presentResult = await runProbePhase(app, runId, 'check_present');
    if (presentResult.error) {
      return {
        passed: false,
        details: `Storage clear presence probe failed: ${presentResult.error}`,
      };
    }

    if (!presentResult.localStoragePresent || !presentResult.indexedDbPresent) {
      return {
        passed: false,
        details:
          'Storage clear presence probe could not observe both localStorage and IndexedDB before clearing.',
      };
    }

    await clearAppRuntimeBrowsingData(app);

    const absentResult = await runProbePhase(app, runId, 'check_absent');
    if (absentResult.error) {
      return {
        passed: false,
        details: `Storage clear absence probe failed: ${absentResult.error}`,
      };
    }

    const passed =
      !absentResult.localStoragePresent && !absentResult.indexedDbPresent;

    return {
      passed,
      details: passed
        ? 'Storage clear cycle removed localStorage and IndexedDB for the target app origin.'
        : 'Storage clear cycle failed because data was still visible after host-side clearing.',
    };
  } catch (err) {
    return {
      passed: false,
      details: err instanceof Error ? err.message : String(err),
    };
  }
}
