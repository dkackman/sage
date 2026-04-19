import type { InstalledSageApp, SandboxState } from '@/bindings';
import { evaluateAppLaunchGate } from '@/lib/apps/sandbox';

export interface SandboxLaunchDecision {
  allowed: boolean;
  title: string;
  description: string;
}

export function getSandboxLaunchDecision(args: {
  app: InstalledSageApp;
  sandboxState: SandboxState | null | undefined;
}): SandboxLaunchDecision {
  const { app, sandboxState } = args;

  if (!sandboxState) {
    return {
      allowed: false,
      title: 'Sandbox tests are still running',
      description:
        'Apps are allowed to launch only when all required sandbox capabilities have passed.',
    };
  }

  const gate = evaluateAppLaunchGate(app, sandboxState);

  if (gate.allowed) {
    return {
      allowed: true,
      title: 'Sandbox checks passed',
      description: 'This app is allowed to launch.',
    };
  }

  if (gate.kind === 'running') {
    return {
      allowed: false,
      title: 'Sandbox tests are still running',
      description:
        gate.message ||
        'Apps are allowed to launch only when all required sandbox capabilities have passed.',
    };
  }

  return {
    allowed: false,
    title: 'Sandbox test failed',
    description:
      gate.message ||
      'This app cannot be launched because a required sandbox capability failed.',
  };
}
