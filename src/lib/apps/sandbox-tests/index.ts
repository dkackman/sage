import type { SandboxState } from '@/lib/apps/sandbox';
import { buildCompletedSandboxState } from '@/lib/apps/sandbox';
import { runIsolationTest } from './isolation';
import { runNetworkTest } from './network';
import { runPersistenceTest } from './persistence';

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
