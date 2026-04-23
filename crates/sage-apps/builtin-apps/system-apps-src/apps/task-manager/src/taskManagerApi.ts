import {
  getSageSystemClient,
  type RuntimeManagerRuntimesChangedEvent,
  type RuntimeTargetParams,
  type SageAppRuntimeRecord,
  type SystemKillRuntimeResult,
} from '@sage-system-app/sdk';

const client = await getSageSystemClient();

export type { SageAppRuntimeRecord as RuntimeRecord };

export function onRuntimesChanged(
  handler: (event: RuntimeManagerRuntimesChangedEvent) => void,
): () => void {
  return client.runtimeManager.onRuntimesChanged(handler);
}

export async function listRuntimes(): Promise<SageAppRuntimeRecord[]> {
  return await client.runtimeManager.listRuntimes();
}

export async function focusRuntime(
  appId: string,
): Promise<SageAppRuntimeRecord> {
  return await client.runtimeManager.focusRuntime({
    appId,
  } satisfies RuntimeTargetParams);
}

export async function hideRuntime(
  appId: string,
): Promise<SageAppRuntimeRecord> {
  return await client.runtimeManager.hideRuntime({
    appId,
  } satisfies RuntimeTargetParams);
}

export async function killRuntime(
  appId: string,
): Promise<SystemKillRuntimeResult> {
  return await client.runtimeManager.killRuntime({
    appId,
  } satisfies RuntimeTargetParams);
}
