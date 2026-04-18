export type SageBridgeVersion = 'v1';

export type SageNetworkPermission = {
  scheme: string;
  host: string;
  required: boolean;
};

export type SageRequestedPermissions = {
  required: string[];
  optional: string[];
};

export type SageAppInfo = {
  id: string;
  name: string;
  version: string;
  requestedPermissions: SageRequestedPermissions;
  grantedPermissions: string[];
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

export type SageBridgeEventPayload = {
  sourceLabel: string;
  request: SageBridgeRequest;
};

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

export type SageWalletClient = {
  sendXch(input: SageWalletSendXchRequest): Promise<TransactionResponse>;
};

export type SageAppClient = {
  bridgePing(): Promise<unknown>;
  bridgeSend(input: SageBridgeSendPayload): Promise<unknown>;
  getInfo(): Promise<SageAppInfo>;
  getPermissions(): Promise<string[]>;
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
