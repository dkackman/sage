import type {
  RuntimeTargetParams,
  SageAppRuntimeRecord,
  SageSystemClient,
  SystemKillRuntimeResult,
} from '@sage-system-app/sdk';

type SageSystemSdkModule = {
  initSageSystemRuntimeBridge(): boolean;
  createSageSystemClient(): Promise<SageSystemClient>;
};

let clientPromise: Promise<SageSystemClient> | null = null;

function sdkUrl(): string {
  return new URL('../sdk.js', import.meta.url).href;
}

async function loadSdk(): Promise<SageSystemSdkModule> {
  return (await import(/* @vite-ignore */ sdkUrl())) as SageSystemSdkModule;
}

async function getClient(): Promise<SageSystemClient> {
  clientPromise ??= (async () => {
    const sdk = await loadSdk();

    if (!sdk.initSageSystemRuntimeBridge()) {
      throw new Error('Sage system bridge is not available in this runtime.');
    }

    return await sdk.createSageSystemClient();
  })();

  return await clientPromise;
}

export type { SageAppRuntimeRecord as RuntimeRecord };

export async function listRuntimes(): Promise<SageAppRuntimeRecord[]> {
  const client = await getClient();
  return await client.runtimeManager.listRuntimes();
}

export async function focusRuntime(
  appId: string,
): Promise<SageAppRuntimeRecord> {
  const client = await getClient();
  return await client.runtimeManager.focusRuntime({
    appId,
  } satisfies RuntimeTargetParams);
}

export async function hideRuntime(
  appId: string,
): Promise<SageAppRuntimeRecord> {
  const client = await getClient();
  return await client.runtimeManager.hideRuntime({
    appId,
  } satisfies RuntimeTargetParams);
}

export async function killRuntime(
  appId: string,
): Promise<SystemKillRuntimeResult> {
  const client = await getClient();
  return await client.runtimeManager.killRuntime({
    appId,
  } satisfies RuntimeTargetParams);
}
