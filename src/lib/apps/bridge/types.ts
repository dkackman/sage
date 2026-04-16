import type {
  Amount,
  InstalledSageApp,
  SageStorageClearRequest,
  SageStorageCountRequest,
  SageStorageCreateIndexRequest,
  SageStorageCreateObjectStoreRequest,
  SageStorageDeleteRequest,
  SageStorageGetAllFromIndexRequest,
  SageStorageGetAllRequest,
  SageStorageGetRequest,
  SageStorageOpenDatabaseRequest,
  SageStoragePutRequest,
} from '@/bindings';

export interface SageBridgeSuccessResponse {
  channel: 'sage-bridge';
  id: string;
  ok: true;
  result: unknown;
}

export interface SageBridgeErrorResponse {
  channel: 'sage-bridge';
  id: string;
  ok: false;
  error: {
    code: string;
    message: string;
  };
}

export type SageBridgeResponse =
  | SageBridgeSuccessResponse
  | SageBridgeErrorResponse;

export interface SageBridgeContext {
  app: InstalledSageApp;
  sourceLabel: string;
}

export interface SageBridgeEventPayload {
  sourceLabel: string;
  request: SageBridgeRequest;
}

export interface SageWalletSendXchRequest {
  address: string;
  amount: Amount;
  fee: Amount;
  memos?: string[];
  clawback?: number | null;
}

export interface SageBridgeRequestParamsMap {
  'bridge.ping': undefined;
  'app.getInfo': undefined;
  'sage.getPermissions': undefined;

  'storage.openDatabase': SageStorageOpenDatabaseRequest;
  'storage.deleteDatabase': string;
  'storage.describeDatabase': string;
  'storage.createObjectStore': SageStorageCreateObjectStoreRequest;
  'storage.createIndex': SageStorageCreateIndexRequest;
  'storage.get': SageStorageGetRequest;
  'storage.put': SageStoragePutRequest;
  'storage.delete': SageStorageDeleteRequest;
  'storage.clear': SageStorageClearRequest;
  'storage.count': SageStorageCountRequest;
  'storage.getAll': SageStorageGetAllRequest;
  'storage.getAllFromIndex': SageStorageGetAllFromIndexRequest;

  'wallet.sendXch': SageWalletSendXchRequest;
}

export type SageBridgeMethod = keyof SageBridgeRequestParamsMap;

export type SageBridgeRequestForMethod<M extends SageBridgeMethod> =
  SageBridgeRequestParamsMap[M] extends undefined
    ? {
        channel: 'sage-bridge';
        id: string;
        method: M;
      }
    : {
        channel: 'sage-bridge';
        id: string;
        method: M;
        params: SageBridgeRequestParamsMap[M];
      };

export type SageBridgeRequest = {
  [M in SageBridgeMethod]: SageBridgeRequestForMethod<M>;
}[SageBridgeMethod];

export type BridgePermissionPolicy =
  | { kind: 'none' }
  | {
      kind: 'custom';
      check: (args: {
        ctx: SageBridgeContext;
        request: SageBridgeRequest;
      }) => void | Promise<void>;
    };

export interface BridgeApprovalRequest {
  kind: 'send_xch';
  app: InstalledSageApp;
  sourceLabel: string;
  requestId: string;
  params: SageWalletSendXchRequest;
}

export type BridgeApprovalResult =
  | { approved: true }
  | { approved: false; reason?: string };

export interface SageBridgeHostTools {
  requestApproval: (
    request: BridgeApprovalRequest,
  ) => Promise<BridgeApprovalResult>;
}

export type BridgeMethodRequest<M extends SageBridgeMethod> =
  SageBridgeRequestForMethod<M>;

export interface BridgeMethodHandlerArgs<M extends SageBridgeMethod> {
  ctx: SageBridgeContext;
  request: BridgeMethodRequest<M>;
  tools: SageBridgeHostTools;
}

export type BridgeApprovalPolicy<M extends SageBridgeMethod> = (args: {
  ctx: SageBridgeContext;
  request: SageBridgeRequestForMethod<M>;
}) => BridgeApprovalRequest | null | Promise<BridgeApprovalRequest | null>;

export interface BridgeMethodHandlerArgs<M extends SageBridgeMethod> {
  ctx: SageBridgeContext;
  request: SageBridgeRequestForMethod<M>;
  tools: SageBridgeHostTools;
}

export interface BridgeMethodDefinition<M extends SageBridgeMethod> {
  permission?: BridgePermissionPolicy;
  approval?: BridgeApprovalPolicy<M>;
  handle: (args: BridgeMethodHandlerArgs<M>) => Promise<unknown>;
}

export type BridgeMethodRegistry = Partial<{
  [M in SageBridgeMethod]: BridgeMethodDefinition<M>;
}>;
