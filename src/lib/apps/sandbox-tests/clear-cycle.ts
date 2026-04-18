import { TEST_APP_IDS } from '@/lib/apps/testApps';
import { getBuiltinApp } from '@/lib/apps/registry';
import { runStorageClearCycle } from '@/lib/apps/storageClearCycle';

export async function runStorageClearCycleCapabilityTest(): Promise<{
  passed: boolean;
  details: string | null;
}> {
  const app = await getBuiltinApp(TEST_APP_IDS.persistencePersistent);

  if (!app) {
    return {
      passed: false,
      details:
        'Missing builtin persistent storage test app for clear-cycle capability test.',
    };
  }

  return runStorageClearCycle(app);
}
