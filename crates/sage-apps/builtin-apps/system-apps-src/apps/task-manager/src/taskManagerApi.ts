export type RuntimeRecord = {
  runtimeId: string;
  runtimeKind: 'user' | 'system';
  appId: string;
  appName: string;
  mode: string;
  state: string;
  startedAt: number;
  visible: boolean;
  internal: boolean;
  activeBatchCount: number;
  activeSocketCount: number;
  inFlightRequestCount: number;
};

async function sleep(ms: number) {
  await new Promise((resolve) => window.setTimeout(resolve, ms));
}

export async function listRuntimes(): Promise<RuntimeRecord[]> {
  await sleep(80);

  return [
    {
      runtimeId: 'mock-runtime-1',
      runtimeKind: 'user',
      appId: 'example-user-app',
      appName: 'Example User App',
      mode: 'inline',
      state: 'running',
      startedAt: Date.now() - 42_000,
      visible: true,
      internal: false,
      activeBatchCount: 0,
      activeSocketCount: 1,
      inFlightRequestCount: 0,
    },
  ];
}

export async function focusRuntime(_appId: string): Promise<void> {
  await sleep(60);
}

export async function hideRuntime(_appId: string): Promise<void> {
  await sleep(60);
}

export async function killRuntime(_appId: string): Promise<void> {
  await sleep(60);
}

