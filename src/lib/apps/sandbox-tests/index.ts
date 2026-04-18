// src/lib/apps/sandbox-tests/index.ts

import type {
  SandboxCapability,
  SandboxCapabilityResult,
  SandboxState,
} from '@/lib/apps/sandbox';
import { buildRunningSandboxState } from '@/lib/apps/sandbox';
import { runStorageClearCycleCapabilityTest } from './clear-cycle';
import { runIsolationTest } from './isolation';
import { runNetworkTest } from './network';
import { runPersistenceTest } from './persistence';

type UpdateFn = (state: SandboxState) => void;

function setCapability(
  state: SandboxState,
  capability: SandboxCapability,
  result: SandboxCapabilityResult,
) {
  state.capabilities[capability] = {
    ...result,
    checkedAt: Date.now(),
  };
}

export async function runSandboxTestsIncremental(
  onUpdate: UpdateFn,
): Promise<SandboxState> {
  const state = buildRunningSandboxState();
  onUpdate({ ...state });

  // --- isolation ---
  const isolation = await runIsolationTest();
  setCapability(state, 'storage_isolation_from_sage', {
    status: isolation.passed ? 'passed' : 'failed',
    checkedAt: Date.now(),
    details: isolation.details,
  });
  onUpdate({ ...state });

  if (!isolation.passed) {
    state.overallCriticalStatus = 'failed';
    state.finishedAt = Date.now();
    onUpdate({ ...state });
    return state;
  }

  // --- persistence ---
  const persistence = await runPersistenceTest();

  setCapability(state, 'storage_persistence_normal', {
    status: persistence.persistentNormal.passed ? 'passed' : 'failed',
    checkedAt: Date.now(),
    details: persistence.persistentNormal.details,
  });

  setCapability(state, 'storage_non_persistence_incognito', {
    status: persistence.persistenceIncognito.passed ? 'passed' : 'failed',
    checkedAt: Date.now(),
    details: persistence.persistenceIncognito.details,
  });

  onUpdate({ ...state });

  // --- clear cycle ---
  const clearCycle = await runStorageClearCycleCapabilityTest();
  setCapability(state, 'storage_clear_cycle', {
    status: clearCycle.passed ? 'passed' : 'failed',
    checkedAt: Date.now(),
    details: clearCycle.details,
  });

  onUpdate({ ...state });

  // --- network ---
  const network = await runNetworkTest();
  setCapability(state, 'network_allowlist_enforced', {
    status: network.passed ? 'passed' : 'failed',
    checkedAt: Date.now(),
    details: network.details,
  });

  // --- finalize ---
  state.overallCriticalStatus = 'passed';
  state.finishedAt = Date.now();

  onUpdate({ ...state });

  return state;
}
