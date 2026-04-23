export * from './generated-types';

import type {
  RuntimeManagerRuntimesChangedEvent,
  RuntimeTargetParams,
  SageAppRuntimeRecord,
  SystemKillRuntimeResult,
} from './generated-types';

export type SageSystemBridgeChannel = 'sage-system-bridge';
export type SageSystemBridgeVersion = 'v1';

export type SageSystemBridgeSuccessResponse = {
  channel: SageSystemBridgeChannel;
  bridgeVersion: SageSystemBridgeVersion;
  id: string;
  ok: true;
  result: unknown;
};

export type SageSystemBridgeErrorResponse = {
  channel: SageSystemBridgeChannel;
  bridgeVersion: SageSystemBridgeVersion;
  id: string;
  ok: false;
  error: {
    code: string;
    message: string;
  };
};

export type SageSystemBridgeResponse =
  | SageSystemBridgeSuccessResponse
  | SageSystemBridgeErrorResponse;

export type SageSystemRuntimeManagerClient = {
  listRuntimes(): Promise<SageAppRuntimeRecord[]>;
  focusRuntime(input: RuntimeTargetParams): Promise<SageAppRuntimeRecord>;
  hideRuntime(input: RuntimeTargetParams): Promise<SageAppRuntimeRecord>;
  killRuntime(input: RuntimeTargetParams): Promise<SystemKillRuntimeResult>;
  onRuntimesChanged(
    handler: (event: RuntimeManagerRuntimesChangedEvent) => void,
  ): () => void;
};

export type SageSystemClient = {
  runtimeManager: SageSystemRuntimeManagerClient;
};
