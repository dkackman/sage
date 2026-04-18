import { TEST_APP_IDS } from '@/lib/apps/testApps';
import {
  startInternalInlineRuntime,
  closeAppRuntime,
} from '@/lib/apps/runtimeRegistry';
import { getBuiltinApp } from '@/lib/apps/registry';
import type { SandboxAppResult } from '@/lib/apps/sandboxRuntimeStore';
import type {
  SandboxIsolationProbeResult,
  SandboxNetworkProbeResult,
  SandboxPersistenceReadProbeResult,
  SandboxPersistenceWriteProbeResult,
  SandboxState,
} from '@/lib/apps/sandbox';
import { buildCompletedSandboxState } from '@/lib/apps/sandbox';
import {
  getSandboxRunResults,
  resetSandboxRun,
} from '@/lib/apps/sandboxRuntimeStore';

const PROBE_LOCAL_STORAGE_KEY = 'sage_probe_local_storage';
const PROBE_INDEXED_DB_NAME = 'sage_probe_db';
const PROBE_INDEXED_DB_STORE = 'probe_store';
const PROBE_INDEXED_DB_KEY = 'sage_probe_key';

function uniqueRunId(prefix: string): string {
  return `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

function formatUnknownError(err: unknown): string {
  if (err instanceof Error) {
    return err.message;
  }

  if (typeof err === 'string') {
    return err;
  }

  try {
    return JSON.stringify(err, null, 2);
  } catch {
    return String(err);
  }
}

function labelForAppId(appId: string): string {
  switch (appId) {
    case TEST_APP_IDS.storageIsolationPersistent:
      return 'persistent isolation';
    case TEST_APP_IDS.storageIsolationIncognito:
      return 'incognito isolation';
    case TEST_APP_IDS.persistencePersistent:
      return 'persistent write/read';
    case TEST_APP_IDS.persistenceIncognito:
      return 'incognito write/read';
    case TEST_APP_IDS.networkAllowA:
      return 'network allow-a';
    case TEST_APP_IDS.networkAllowB:
      return 'network allow-b';
    default:
      return appId;
  }
}

function shouldKeepFailedTestAppsOpen(): boolean {
  return (
    import.meta.env.DEV && import.meta.env.VITE_SAGE_DEBUG_TEST_APPS === '1'
  );
}

async function stopTestAppsOnSuccess(appIds: string[]): Promise<void> {
  await stopTestApps(appIds);
}

async function stopTestAppsOnFailure(appIds: string[]): Promise<void> {
  if (shouldKeepFailedTestAppsOpen()) {
    return;
  }

  await stopTestApps(appIds);
}

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

function findByAppId<T>(
  items: SandboxAppResult<T>[],
  appId: string,
): T | undefined {
  return items.find((item) => item.appId === appId)?.data;
}

async function writeSageProbeIndexedDb(runId: string): Promise<void> {
  await new Promise<void>((resolve, reject) => {
    const open = indexedDB.open(PROBE_INDEXED_DB_NAME);

    open.onerror = () => {
      reject(new Error('Failed to open Sage probe IndexedDB.'));
    };

    open.onupgradeneeded = () => {
      const db = open.result;
      if (!db.objectStoreNames.contains(PROBE_INDEXED_DB_STORE)) {
        db.createObjectStore(PROBE_INDEXED_DB_STORE);
      }
    };

    open.onsuccess = () => {
      try {
        const db = open.result;
        const tx = db.transaction(PROBE_INDEXED_DB_STORE, 'readwrite');
        const store = tx.objectStore(PROBE_INDEXED_DB_STORE);
        const put = store.put(runId, PROBE_INDEXED_DB_KEY);

        put.onerror = () => {
          db.close();
          reject(new Error('Failed to write Sage probe IndexedDB record.'));
        };

        put.onsuccess = () => {
          tx.oncomplete = () => {
            db.close();
            resolve();
          };

          tx.onerror = () => {
            db.close();
            reject(
              new Error('Failed to commit Sage probe IndexedDB transaction.'),
            );
          };
        };
      } catch (err) {
        reject(err instanceof Error ? err : new Error(String(err)));
      }
    };
  });
}

async function writeSageProbes(runId: string): Promise<void> {
  localStorage.setItem(PROBE_LOCAL_STORAGE_KEY, runId);
  await writeSageProbeIndexedDb(runId);
}

async function startTestApp(
  appId: string,
  query?: Record<string, string>,
): Promise<void> {
  const app = await getBuiltinApp(appId);
  if (!app) {
    throw new Error(`Missing builtin test app ${appId}`);
  }

  await startInternalInlineRuntime(app, { query });
}

async function stopTestApps(appIds: string[]): Promise<void> {
  await Promise.allSettled(appIds.map((appId) => closeAppRuntime(appId)));
}

async function pollResults<T>(args: {
  runId: string;
  expectedCount: number;
  timeoutMs: number;
  read: () => T[];
  label: string;
}): Promise<T[]> {
  const startedAt = Date.now();

  for (;;) {
    const results = args.read();

    if (results.length >= args.expectedCount) {
      return results;
    }

    if (Date.now() - startedAt >= args.timeoutMs) {
      throw new Error(`Timed out waiting for ${args.label} results.`);
    }

    await new Promise((resolve) => window.setTimeout(resolve, 100));
  }
}

async function runIsolationTest(): Promise<{
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

async function runPersistenceTest(): Promise<{
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

async function runNetworkTest(): Promise<{
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

export async function runSandboxTests(): Promise<SandboxState> {
  const isolation = await runIsolationTest();

  if (!isolation.passed) {
    return buildCompletedSandboxState({
      isolation,
      persistenceNormal: {
        passed: false,
        details: 'Skipped because critical storage isolation baseline failed.',
      },
      persistenceIncognito: {
        passed: false,
        details: 'Skipped because critical storage isolation baseline failed.',
      },
      network: {
        passed: false,
        details: 'Skipped because critical storage isolation baseline failed.',
      },
    });
  }

  const persistence = await runPersistenceTest();
  const network = await runNetworkTest();

  return buildCompletedSandboxState({
    isolation,
    persistenceNormal: persistence.persistentNormal,
    persistenceIncognito: persistence.persistenceIncognito,
    network,
  });
}
