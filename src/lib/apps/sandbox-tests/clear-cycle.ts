import { TEST_APP_IDS } from '@/lib/apps/testApps';
import { getBuiltinApp } from '@/lib/apps/registry';
import { clearAppDataStore } from '@/lib/apps/storageClearCycle';

export async function runStorageClearCycleCapabilityTest(): Promise<{
  passed: boolean;
  details: string | null;
}> {
  const app = await getBuiltinApp(TEST_APP_IDS.storageClearPersistent);

  if (!app) {
    return {
      passed: false,
      details:
        'Missing builtin storage clear test app for clear-cycle capability test.',
    };
  }

  return clearAppDataStore(app);
}
