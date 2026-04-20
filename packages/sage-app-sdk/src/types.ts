export type SageBridgeVersion = 'v1';

export type SageNetworkPermission = {
  scheme: string;
  host: string;
  required: boolean;
};

export type SageRequestedPermissions = {
  network?: SageRequestedNetworkPermissions;
  capabilities?: SageRequestedCapabilities;
};

export type SageNetworkWhitelistEntry = {
  scheme: string;
  host: string;
  required?: boolean;
};

export type SageRequestedCapabilities = {
  required?: string[];
  optional?: string[];
};

export type SageRequestedNetworkPermissions = {
  whitelist?: SageRequestedNetworkWhitelist;
};

export type SageRequestedNetworkWhitelist = {
  required?: SageRequestedNetworkWhitelistEntry[];
  optional?: SageRequestedNetworkWhitelistEntry[];
};

export type SageRequestedNetworkWhitelistEntry = {
  scheme: string;
  host: string;
};

export type SageAppInfo = {
  id: string;
  name: string;
  version: string;
  requestedPermissions: SageRequestedPermissions;
  capabilities: string[];
  network: SageNetworkPermission[];
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

export type SageGrantedCapabilitiesChangeEvent = {
  channel: 'sage-bridge';
  type: 'grantedCapabilitiesChange';
  removedGrantedCapabilities: string[];
  addedGrantedCapabilities: string[];
  fullGrantedCapabilities: string[];
};

export type SageGrantedNetworkWhitelistChangeEvent = {
  channel: 'sage-bridge';
  type: 'grantedNetworkWhitelistChange';
  removedGrantedNetworkWhitelist: SageRequestedNetworkWhitelistEntry[];
  addedGrantedNetworkWhitelist: SageRequestedNetworkWhitelistEntry[];
  fullGrantedNetworkWhitelist: SageRequestedNetworkWhitelistEntry[];
};

export type SageBridgeRuntimeEvent =
  | SageGrantedCapabilitiesChangeEvent
  | SageGrantedNetworkWhitelistChangeEvent;

export type SageBridgeSendPayload = {
  kind: string;
  [key: string]: unknown;
};

export type SageLifecycleBeforeStopDetail = {
  reason?: string;
  appId?: string;
  runtimeId?: string;
};

export type AssetKind = 'token' | 'nft' | 'did' | 'option';
export type Amount = string | number;

export type Asset = {
  asset_id: string | null;
  name: string | null;
  ticker: string | null;
  precision: number;
  icon_url: string | null;
  description: string | null;
  is_sensitive_content: boolean;
  is_visible: boolean;
  revocation_address: string | null;
  kind: AssetKind;
};

export type TransactionInput = {
  coin_id: string;
  amount: Amount;
  address: string;
  asset: Asset | null;
  outputs: TransactionOutput[];
};

export type TransactionOutput = {
  coin_id: string;
  amount: Amount;
  address: string;
  receiving: boolean;
  burning: boolean;
};

export type TransactionSummary = {
  fee: Amount;
  inputs: TransactionInput[];
};

export type CoinJson = {
  parent_coin_info: string;
  puzzle_hash: string;
  amount: Amount;
};

export type CoinSpendJson = {
  coin: CoinJson;
  puzzle_reveal: string;
  solution: string;
};

export type TransactionResponse = {
  summary: TransactionSummary;
  coin_spends: CoinSpendJson[];
};

export type SageWalletSendXchRequest = {
  address: string;
  amount: Amount;
  fee: Amount;
  memos?: string[];
  clawback?: number | null;
  auto_submit?: boolean;
};

export type SageRequestCapabilityGrantInput = {
  capability: string;
};

export type SageRequestCapabilityGrantResult = {
  granted: boolean;
  alreadyGranted?: boolean;
  capability: string;
  fullGrantedCapabilities: string[];
};

export type SageRequestNetworkWhitelistGrantInput = {
  entry: SageRequestedNetworkWhitelistEntry;
};

export type SageRequestNetworkWhitelistGrantResult = {
  granted: boolean;
  alreadyGranted?: boolean;
  entry: SageRequestedNetworkWhitelistEntry;
  fullGrantedNetworkWhitelist: SageRequestedNetworkWhitelistEntry[];
};

export type SageWalletClient = {
  sendXch(input: SageWalletSendXchRequest): Promise<TransactionResponse>;
};

export type SageAppClient = {
  bridgePing(): Promise<unknown>;
  bridgeSend(input: SageBridgeSendPayload): Promise<unknown>;
  getInfo(): Promise<SageAppInfo>;
  getCapabilities(): Promise<string[]>;
  requestCapabilityGrant(
    input: SageRequestCapabilityGrantInput,
  ): Promise<SageRequestCapabilityGrantResult>;
  requestNetworkWhitelistGrant(
    input: SageRequestNetworkWhitelistGrantInput,
  ): Promise<SageRequestNetworkWhitelistGrantResult>;
  onGrantedCapabilitiesChange(
    handler: (event: SageGrantedCapabilitiesChangeEvent) => void,
  ): () => void;
  onGrantedNetworkWhitelistChange(
    handler: (event: SageGrantedNetworkWhitelistChangeEvent) => void,
  ): () => void;
};

export type SageLifecycleClient = {
  onBeforeStop(
    handler: (detail: SageLifecycleBeforeStopDetail) => void,
  ): () => void;
};

export type SageClient = {
  initialAppInfo: SageAppInfo;
  app: SageAppClient;
  lifecycle: SageLifecycleClient;
  wallet: SageWalletClient;
};

