import { TEST_APP_IDS } from '@/lib/apps/testApps';
import {
  startInternalInlineRuntime,
  closeAppRuntime,
} from '@/lib/apps/runtimeRegistry';
import { getBuiltinApp } from '@/lib/apps/registry';
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
const PROBE_COOKIE_KEY = 'sage_probe_cookie';
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
  document.cookie = `${PROBE_COOKIE_KEY}=${encodeURIComponent(runId)}; path=/`;
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

    const results = await pollResults<SandboxIsolationProbeResult>({
      runId,
      expectedCount: 2,
      timeoutMs: 10000,
      label: 'sandbox isolation',
      read: () => getSandboxRunResults(runId).isolation,
    });

    if (results.length !== 2) {
      return {
        passed: false,
        details: `Expected 2 isolation probe results, got ${results.length}.`,
      };
    }

    const modes = new Set(results.map((result) => result.mode));
    if (!modes.has('persistent') || !modes.has('incognito')) {
      return {
        passed: false,
        details: 'Isolation probe returned incomplete or malformed mode set.',
      };
    }

    for (const result of results) {
      if (result.runId !== runId) {
        return {
          passed: false,
          details: `Isolation probe ${result.mode} reported mismatched run id.`,
        };
      }

      if (result.error) {
        return {
          passed: false,
          details: `Isolation probe ${result.mode} reported error: ${result.error}`,
        };
      }

      if (
        result.localStorageVisible ||
        result.cookieVisible ||
        result.indexedDbVisible
      ) {
        return {
          passed: false,
          details: `Isolation probe ${result.mode} was able to observe Sage probe data.`,
        };
      }
    }

    return {
      passed: true,
      details:
        'Both sandbox probe modes were unable to observe Sage probe data.',
    };
  } catch (err) {
    return {
      passed: false,
      details: formatUnknownError(err),
    };
  } finally {
    await stopTestApps(appIds);
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

    const writeResults = await pollResults<SandboxPersistenceWriteProbeResult>({
      runId,
      expectedCount: 2,
      timeoutMs: 10000,
      label: 'sandbox persistence write',
      read: () => getSandboxRunResults(runId).persistenceWrite,
    });

    if (writeResults.length !== 2) {
      const details = `Expected 2 persistence write results, got ${writeResults.length}.`;
      return {
        persistentNormal: { passed: false, details },
        persistenceIncognito: { passed: false, details },
      };
    }

    const persistentWrite = writeResults.find((r) => r.mode === 'persistent');
    const incognitoWrite = writeResults.find((r) => r.mode === 'incognito');

    if (!persistentWrite || !incognitoWrite) {
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

    if (
      !persistentWrite.localStorageWrote ||
      !persistentWrite.cookieWrote ||
      !persistentWrite.indexedDbWrote
    ) {
      return {
        persistentNormal: {
          passed: false,
          details:
            'Persistent write probe could not write all persistence surfaces.',
        },
        persistenceIncognito: {
          passed: false,
          details:
            'Persistent write probe could not write all persistence surfaces.',
        },
      };
    }

    if (incognitoWrite.error) {
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

    if (
      !incognitoWrite.localStorageWrote ||
      !incognitoWrite.cookieWrote ||
      !incognitoWrite.indexedDbWrote
    ) {
      return {
        persistentNormal: {
          passed: false,
          details:
            'Incognito write probe could not write all persistence surfaces.',
        },
        persistenceIncognito: {
          passed: false,
          details:
            'Incognito write probe could not write all persistence surfaces.',
        },
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

    const readResults = await pollResults<SandboxPersistenceReadProbeResult>({
      runId,
      expectedCount: 2,
      timeoutMs: 10000,
      label: 'sandbox persistence read',
      read: () => getSandboxRunResults(runId).persistenceRead,
    });

    if (readResults.length !== 2) {
      const details = `Expected 2 persistence read results, got ${readResults.length}.`;
      return {
        persistentNormal: { passed: false, details },
        persistenceIncognito: { passed: false, details },
      };
    }

    const persistentRead = readResults.find((r) => r.mode === 'persistent');
    const incognitoRead = readResults.find((r) => r.mode === 'incognito');

    if (!persistentRead || !incognitoRead) {
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
      persistentRead.cookiePresent &&
      persistentRead.indexedDbPresent;

    const incognitoPassed =
      !incognitoRead.error &&
      !incognitoRead.localStoragePresent &&
      !incognitoRead.cookiePresent &&
      !incognitoRead.indexedDbPresent;

    return {
      persistentNormal: {
        passed: persistentPassed,
        details: persistentPassed
          ? 'Persistent mode retained localStorage, cookies, and IndexedDB across reopen.'
          : persistentRead.error
            ? `Persistent read probe reported error: ${persistentRead.error}`
            : 'Persistent mode did not retain all persistence surfaces across reopen.',
      },
      persistenceIncognito: {
        passed: incognitoPassed,
        details: incognitoPassed
          ? 'Incognito mode did not retain localStorage, cookies, or IndexedDB across reopen.'
          : incognitoRead.error
            ? `Incognito read probe reported error: ${incognitoRead.error}`
            : 'Incognito mode retained browser data across reopen when it should not have.',
      },
    };
  } catch (err) {
    const message = formatUnknownError(err);

    return {
      persistentNormal: { passed: false, details: message },
      persistenceIncognito: { passed: false, details: message },
    };
  } finally {
    await stopTestApps(appIds);
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

    const results = await pollResults<SandboxNetworkProbeResult>({
      runId,
      expectedCount: 2,
      timeoutMs: 12000,
      label: 'sandbox network',
      read: () => getSandboxRunResults(runId).network,
    });

    if (results.length !== 2) {
      return {
        passed: false,
        details: `Expected 2 network probe results, got ${results.length}.`,
      };
    }

    const modes = new Set(results.map((result) => result.mode));
    if (!modes.has('allow-a') || !modes.has('allow-b')) {
      return {
        passed: false,
        details: 'Network probe returned incomplete or malformed mode set.',
      };
    }

    for (const result of results) {
      if (result.runId !== runId) {
        return {
          passed: false,
          details: `Network probe ${result.mode} reported mismatched run id.`,
        };
      }

      if (result.error) {
        return {
          passed: false,
          details: `Network probe ${result.mode} reported error: ${result.error}`,
        };
      }

      if (!result.allowedOk) {
        return {
          passed: false,
          details: `Network probe ${result.mode} could not reach its allowed URL ${result.allowedUrl}.`,
        };
      }

      if (result.blockedOk) {
        return {
          passed: false,
          details: `Network probe ${result.mode} was able to reach blocked URL ${result.blockedUrl}.`,
        };
      }
    }

    return {
      passed: true,
      details:
        'Network allowlist probes succeeded for allowed URLs and failed for blocked URLs in both flipped configurations.',
    };
  } catch (err) {
    return {
      passed: false,
      details: formatUnknownError(err),
    };
  } finally {
    await stopTestApps(appIds);
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
