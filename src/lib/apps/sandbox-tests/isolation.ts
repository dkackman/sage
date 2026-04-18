import { TEST_APP_IDS } from '@/lib/apps/testApps';
import type { SandboxIsolationProbeResult } from '@/lib/apps/sandbox';
import type { SandboxAppResult } from '@/lib/apps/sandboxRuntimeStore';
import {
  getSandboxRunResults,
  resetSandboxRun,
} from '@/lib/apps/sandboxRuntimeStore';
import { writeSageProbes } from './sageProbeState';
import { findByAppId } from './lookup';
import {
  formatUnknownError,
  labelForAppId,
  pollResults,
  startTestApp,
  stopTestApps,
  stopTestAppsOnFailure,
  stopTestAppsOnSuccess,
  uniqueRunId,
} from './shared';

export async function runIsolationTest(): Promise<{
  passed: boolean;
  details: string | null;
}> {
  const runId = uniqueRunId('sandbox-isolation');
  const appIds = [
    TEST_APP_IDS.storageIsolationPersistent,
    TEST_APP_IDS.storageIsolationIncognito,
  ];

  resetSandboxRun(runId);
  await stopTestApps(appIds);
  await writeSageProbes(runId);

  try {
    await Promise.all([
      startTestApp(TEST_APP_IDS.storageIsolationPersistent, { runId }),
      startTestApp(TEST_APP_IDS.storageIsolationIncognito, { runId }),
    ]);

    const results = await pollResults<
      SandboxAppResult<SandboxIsolationProbeResult>
    >({
      runId,
      expectedCount: 2,
      timeoutMs: 10000,
      label: 'sandbox isolation',
      read: () => getSandboxRunResults(runId).isolation,
    });

    const persistent = findByAppId(
      results,
      TEST_APP_IDS.storageIsolationPersistent,
    );
    const incognito = findByAppId(
      results,
      TEST_APP_IDS.storageIsolationIncognito,
    );

    if (!persistent || !incognito) {
      await stopTestAppsOnFailure(appIds);
      return {
        passed: false,
        details: 'Isolation probe returned incomplete app result set.',
      };
    }

    for (const [appId, result] of [
      [TEST_APP_IDS.storageIsolationPersistent, persistent],
      [TEST_APP_IDS.storageIsolationIncognito, incognito],
    ] as const) {
      const label = labelForAppId(appId);

      if (result.runId !== runId) {
        await stopTestAppsOnFailure(appIds);
        return {
          passed: false,
          details: `${label} probe reported mismatched run id.`,
        };
      }

      if (result.error) {
        await stopTestAppsOnFailure(appIds);
        return {
          passed: false,
          details: `${label} probe reported error: ${result.error}`,
        };
      }

      if (result.localStorageVisible || result.indexedDbVisible) {
        await stopTestAppsOnFailure(appIds);
        return {
          passed: false,
          details: `${label} probe was able to observe Sage probe data.`,
        };
      }
    }

    await stopTestAppsOnSuccess(appIds);

    return {
      passed: true,
      details:
        'Both sandbox probe modes were unable to observe Sage probe data.',
    };
  } catch (err) {
    await stopTestAppsOnFailure(appIds);
    return {
      passed: false,
      details: formatUnknownError(err),
    };
  }
}
