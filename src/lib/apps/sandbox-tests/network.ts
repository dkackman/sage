import { TEST_APP_IDS } from '@/lib/apps/testApps';
import type { SandboxNetworkProbeResult } from '@/lib/apps/sandbox';
import type { SandboxAppResult } from '@/lib/apps/sandboxRuntimeStore';
import {
  getSandboxRunResults,
  resetSandboxRun,
} from '@/lib/apps/sandboxRuntimeStore';
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

export async function runNetworkTest(): Promise<{
  passed: boolean;
  details: string | null;
}> {
  const runId = uniqueRunId('sandbox-network');
  const appIds = [TEST_APP_IDS.networkAllowA, TEST_APP_IDS.networkAllowB];

  resetSandboxRun(runId);
  await stopTestApps(appIds);

  try {
    await Promise.all([
      startTestApp(TEST_APP_IDS.networkAllowA, { runId }),
      startTestApp(TEST_APP_IDS.networkAllowB, { runId }),
    ]);

    const results = await pollResults<
      SandboxAppResult<SandboxNetworkProbeResult>
    >({
      runId,
      expectedCount: 2,
      timeoutMs: 12000,
      label: 'sandbox network',
      read: () => getSandboxRunResults(runId).network,
    });

    const allowA = findByAppId(results, TEST_APP_IDS.networkAllowA);
    const allowB = findByAppId(results, TEST_APP_IDS.networkAllowB);

    if (!allowA || !allowB) {
      await stopTestAppsOnFailure(appIds);
      return {
        passed: false,
        details: 'Network probe returned incomplete app result set.',
      };
    }

    for (const [appId, result] of [
      [TEST_APP_IDS.networkAllowA, allowA],
      [TEST_APP_IDS.networkAllowB, allowB],
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

      if (!result.allowedOk) {
        await stopTestAppsOnFailure(appIds);
        return {
          passed: false,
          details: `${label} probe could not reach its allowed URL ${result.allowedUrl}.`,
        };
      }

      if (result.blockedOk) {
        await stopTestAppsOnFailure(appIds);
        return {
          passed: false,
          details: `${label} probe was able to reach blocked URL ${result.blockedUrl}.`,
        };
      }
    }

    await stopTestAppsOnSuccess(appIds);

    return {
      passed: true,
      details:
        'Network allowlist probes succeeded for allowed URLs and failed for blocked URLs in both flipped configurations.',
    };
  } catch (err) {
    await stopTestAppsOnFailure(appIds);
    return {
      passed: false,
      details: formatUnknownError(err),
    };
  }
}
