import {
  AppGetInfoResult,
  BridgePingResult,
  BridgeSendResult,
  GrantedCapabilitiesChangeEvent,
  GrantedNetworkWhitelistChangeEvent,
  ReadyToStopParams,
  RequestCapabilityGrantParams,
  RequestCapabilityGrantResult,
  RequestNetworkWhitelistGrantParams,
  RequestNetworkWhitelistGrantResult,
  RuntimeAckResult,
  SageLifecycleBeforeStopDetail,
  SetBeforeStopListenerParams,
  TransactionResponse,
  WalletSendXchParams,
} from './generated-types';

export * from './generated-types';

export type SageBridgeVersion = 'v1';

export type SageBridgeSendPayload = {
  kind: string;
  [key: string]: unknown;
};

export type SageBridgeRequest = {
  channel: 'sage-bridge';
  bridgeVersion?: SageBridgeVersion;
  id: string;
  method: string;
  params?: unknown;
};

export type SageBridgeSuccessResponse = {
  channel: 'sage-bridge';
  bridgeVersion: SageBridgeVersion;
  id: string;
  ok: true;
  result: unknown;
};

export type SageBridgeErrorResponse = {
  channel: 'sage-bridge';
  bridgeVersion: SageBridgeVersion;
  id: string;
  ok: false;
  error: {
    code: string;
    message: string;
  };
};

export type SageBridgeResponse =
  | SageBridgeSuccessResponse
  | SageBridgeErrorResponse;

export type SageBridgeRuntimeEvent =
  | GrantedCapabilitiesChangeEvent
  | GrantedNetworkWhitelistChangeEvent;

export type SageWalletClient = {
  sendXch(input: WalletSendXchParams): Promise<TransactionResponse>;
};

export type SageAppLifecycleClient = {
  onBeforeStop(
    handler: (
      detail: Omit<SageLifecycleBeforeStopDetail, 'requestId'>,
    ) => void | Promise<void>,
  ): () => void;
};

export type SageAppClient = {
  bridgePing(): Promise<BridgePingResult>;
  bridgeSend(input: SageBridgeSendPayload): Promise<BridgeSendResult>;
  getInfo(): Promise<AppGetInfoResult>;
  getCapabilities(): Promise<string[]>;
  requestCapabilityGrant(
    input: RequestCapabilityGrantParams,
  ): Promise<RequestCapabilityGrantResult>;
  requestNetworkWhitelistGrant(
    input: RequestNetworkWhitelistGrantParams,
  ): Promise<RequestNetworkWhitelistGrantResult>;
  onGrantedCapabilitiesChange(
    handler: (event: GrantedCapabilitiesChangeEvent) => void,
  ): () => void;
  onGrantedNetworkWhitelistChange(
    handler: (event: GrantedNetworkWhitelistChangeEvent) => void,
  ): () => void;
  lifecycle: SageAppLifecycleClient;
};

export type SageClient = {
  initialAppInfo: AppGetInfoResult;
  app: SageAppClient;
  wallet: SageWalletClient;
};

export type {
  RuntimeAckResult,
  ReadyToStopParams,
  SetBeforeStopListenerParams,
  SageLifecycleBeforeStopDetail,
};

