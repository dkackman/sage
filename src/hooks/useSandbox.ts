import { useCallback, useEffect, useState } from 'react';
import {
  buildCompletedSandboxState,
  buildInitialSandboxState,
  buildRunningSandboxState,
  type SandboxState,
} from '@/lib/apps/sandbox';
import { runSandboxTests } from '@/lib/apps/runSandboxTests';

export function useSandboxInternal() {
  const [sandboxState, setSandboxState] = useState<SandboxState>(
    buildInitialSandboxState(),
  );

  const rerunSandboxTests = useCallback(async () => {
    setSandboxState(buildRunningSandboxState());

    try {
      const result = await runSandboxTests();
      setSandboxState(result);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);

      setSandboxState(
        buildCompletedSandboxState({
          isolation: {
            passed: false,
            details: message,
          },
          persistenceNormal: {
            passed: false,
            details: 'Skipped because sandbox run failed before completion.',
          },
          persistenceIncognito: {
            passed: false,
            details: 'Skipped because sandbox run failed before completion.',
          },
          network: {
            passed: false,
            details: 'Skipped because sandbox run failed before completion.',
          },
        }),
      );
    }
  }, []);

  useEffect(() => {
    void rerunSandboxTests();
  }, [rerunSandboxTests]);

  return {
    sandboxState,
    rerunSandboxTests,
  };
}
