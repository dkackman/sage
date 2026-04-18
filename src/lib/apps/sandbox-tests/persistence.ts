import { TEST_APP_IDS } from '@/lib/apps/testApps';
import type {
  SandboxPersistenceReadProbeResult,
  SandboxPersistenceWriteProbeResult,
} from '@/lib/apps/sandbox';
import type { SandboxAppResult } from '@/lib/apps/sandboxRuntimeStore';
import {
  getSandboxRunResults,
  resetSandboxRun,
} from '@/lib/apps/sandboxRuntimeStore';
import { findByAppId } from './lookup';
import {
  formatUnknownError,
  pollResults,
  startTestApp,
  stopTestApps,
  stopTestAppsOnFailure,
  stopTestAppsOnSuccess,
  uniqueRunId,
} from './shared';

function formatPersistenceWriteFailure(
  label: string,
  result: SandboxPersistenceWriteProbeResult,
): string {
  const failed: string[] = [];

  if (!result.localStorageWrote) {
    failed.push('localStorage');
  }

  if (!result.indexedDbWrote) {
    failed.push('IndexedDB');
  }

  if (failed.length === 0) {
    return `${label} write probe failed for unknown reason.`;
  }

  return `${label} write probe could not write: ${failed.join(', ')}.`;
}

function formatPersistenceReadFailure(
  label: string,
  result: SandboxPersistenceReadProbeResult,
  expectedPresent: boolean,
): string {
  const mismatches: string[] = [];

  if (result.localStoragePresent !== expectedPresent) {
    mismatches.push(
      `localStorage=${result.localStoragePresent} expected=${expectedPresent}`,
    );
  }

  if (result.indexedDbPresent !== expectedPresent) {
    mismatches.push(
      `IndexedDB=${result.indexedDbPresent} expected=${expectedPresent}`,
    );
  }

  if (mismatches.length === 0) {
    return `${label} read probe failed for unknown reason.`;
  }

  return `${label} read probe mismatch: ${mismatches.join(', ')}.`;
}

export async function runPersistenceTest(): Promise<{
  persistentNormal: { passed: boolean; details: string | null };
  persistenceIncognito: { passed: boolean; details: string | null };
}> {
  const runId = uniqueRunId('sandbox-persistence');
  const appIds = [
    TEST_APP_IDS.persistencePersistent,
    TEST_APP_IDS.persistenceIncognito,
  ];

  resetSandboxRun(runId);
  await stopTestApps(appIds);

  try {
    await Promise.all([
      startTestApp(TEST_APP_IDS.persistencePersistent, {
        runId,
        phase: 'write',
      }),
      startTestApp(TEST_APP_IDS.persistenceIncognito, {
        runId,
        phase: 'write',
      }),
    ]);

    const writeResults = await pollResults<
      SandboxAppResult<SandboxPersistenceWriteProbeResult>
    >({
      runId,
      expectedCount: 2,
      timeoutMs: 10000,
      label: 'sandbox persistence write',
      read: () => getSandboxRunResults(runId).persistenceWrite,
    });

    const persistentWrite = findByAppId(
      writeResults,
      TEST_APP_IDS.persistencePersistent,
    );
    const incognitoWrite = findByAppId(
      writeResults,
      TEST_APP_IDS.persistenceIncognito,
    );

    if (!persistentWrite || !incognitoWrite) {
      await stopTestAppsOnFailure(appIds);
      return {
        persistentNormal: {
          passed: false,
          details: 'Missing persistent or incognito persistence write result.',
        },
        persistenceIncognito: {
          passed: false,
          details: 'Missing persistent or incognito persistence write result.',
        },
      };
    }

    if (persistentWrite.runId !== runId || incognitoWrite.runId !== runId) {
      await stopTestAppsOnFailure(appIds);
      return {
        persistentNormal: {
          passed: false,
          details: 'Persistence write probe reported mismatched run id.',
        },
        persistenceIncognito: {
          passed: false,
          details: 'Persistence write probe reported mismatched run id.',
        },
      };
    }

    if (persistentWrite.error) {
      await stopTestAppsOnFailure(appIds);
      return {
        persistentNormal: {
          passed: false,
          details: `Persistent write probe reported error: ${persistentWrite.error}`,
        },
        persistenceIncognito: {
          passed: false,
          details: `Persistent write probe reported error: ${persistentWrite.error}`,
        },
      };
    }

    if (!persistentWrite.localStorageWrote || !persistentWrite.indexedDbWrote) {
      const details = formatPersistenceWriteFailure(
        'persistent',
        persistentWrite,
      );

      await stopTestAppsOnFailure(appIds);
      return {
        persistentNormal: { passed: false, details },
        persistenceIncognito: { passed: false, details },
      };
    }

    if (incognitoWrite.error) {
      await stopTestAppsOnFailure(appIds);
      return {
        persistentNormal: {
          passed: false,
          details: `Incognito write probe reported error: ${incognitoWrite.error}`,
        },
        persistenceIncognito: {
          passed: false,
          details: `Incognito write probe reported error: ${incognitoWrite.error}`,
        },
      };
    }

    if (!incognitoWrite.localStorageWrote || !incognitoWrite.indexedDbWrote) {
      const details = formatPersistenceWriteFailure(
        'incognito',
        incognitoWrite,
      );

      await stopTestAppsOnFailure(appIds);
      return {
        persistentNormal: { passed: false, details },
        persistenceIncognito: { passed: false, details },
      };
    }

    await stopTestApps(appIds);

    await Promise.all([
      startTestApp(TEST_APP_IDS.persistencePersistent, {
        runId,
        phase: 'read',
      }),
      startTestApp(TEST_APP_IDS.persistenceIncognito, {
        runId,
        phase: 'read',
      }),
    ]);

    const readResults = await pollResults<
      SandboxAppResult<SandboxPersistenceReadProbeResult>
    >({
      runId,
      expectedCount: 2,
      timeoutMs: 10000,
      label: 'sandbox persistence read',
      read: () => getSandboxRunResults(runId).persistenceRead,
    });

    const persistentRead = findByAppId(
      readResults,
      TEST_APP_IDS.persistencePersistent,
    );
    const incognitoRead = findByAppId(
      readResults,
      TEST_APP_IDS.persistenceIncognito,
    );

    if (!persistentRead || !incognitoRead) {
      await stopTestAppsOnFailure(appIds);
      return {
        persistentNormal: {
          passed: false,
          details: 'Missing persistent or incognito persistence read result.',
        },
        persistenceIncognito: {
          passed: false,
          details: 'Missing persistent or incognito persistence read result.',
        },
      };
    }

    if (persistentRead.runId !== runId || incognitoRead.runId !== runId) {
      await stopTestAppsOnFailure(appIds);
      return {
        persistentNormal: {
          passed: false,
          details: 'Persistence read probe reported mismatched run id.',
        },
        persistenceIncognito: {
          passed: false,
          details: 'Persistence read probe reported mismatched run id.',
        },
      };
    }

    const persistentPassed =
      !persistentRead.error &&
      persistentRead.localStoragePresent &&
      persistentRead.indexedDbPresent;

    const incognitoPassed =
      !incognitoRead.error &&
      !incognitoRead.localStoragePresent &&
      !incognitoRead.indexedDbPresent;

    const result = {
      persistentNormal: {
        passed: persistentPassed,
        details: persistentPassed
          ? 'Persistent mode retained localStorage and IndexedDB across reopen.'
          : persistentRead.error
            ? `Persistent read probe reported error: ${persistentRead.error}`
            : formatPersistenceReadFailure('persistent', persistentRead, true),
      },
      persistenceIncognito: {
        passed: incognitoPassed,
        details: incognitoPassed
          ? 'Incognito mode did not retain localStorage or IndexedDB across reopen.'
          : incognitoRead.error
            ? `Incognito read probe reported error: ${incognitoRead.error}`
            : formatPersistenceReadFailure('incognito', incognitoRead, false),
      },
    };

    if (result.persistentNormal.passed && result.persistenceIncognito.passed) {
      await stopTestAppsOnSuccess(appIds);
    } else {
      await stopTestAppsOnFailure(appIds);
    }

    return result;
  } catch (err) {
    await stopTestAppsOnFailure(appIds);
    const message = formatUnknownError(err);

    return {
      persistentNormal: { passed: false, details: message },
      persistenceIncognito: { passed: false, details: message },
    };
  }
}
