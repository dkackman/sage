import type { Amount, InstalledSageApp } from '@/bindings';
import type {
  SandboxIsolationProbeResult,
  SandboxNetworkProbeResult,
  SandboxPersistenceReadProbeResult,
  SandboxPersistenceWriteProbeResult,
} from '@/lib/apps/sandbox';

export type SageBridgeVersion = 'v1';

export interface SageBridgeSuccessResponse {
  channel: 'sage-bridge';
  bridgeVersion: SageBridgeVersion;
  id: string;
  ok: true;
  result: unknown;
}

export interface SageBridgeErrorResponse {
  channel: 'sage-bridge';
  bridgeVersion: SageBridgeVersion;
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

export interface SageBridgeSendPayload {
  kind: 'sandbox_report';
  report:
    | {
        type: 'isolation';
        data: SandboxIsolationProbeResult;
      }
    | {
        type: 'persistence_write';
        data: SandboxPersistenceWriteProbeResult;
      }
    | {
        type: 'persistence_read';
        data: SandboxPersistenceReadProbeResult;
      }
    | {
        type: 'network';
        data: SandboxNetworkProbeResult;
      };
}

export interface SageBridgeRequestParamsMap {
  'bridge.ping': undefined;
  'bridge.send': SageBridgeSendPayload;
  'app.getInfo': undefined;
  'sage.getPermissions': undefined;
  'wallet.sendXch': SageWalletSendXchRequest;
}

export type SageBridgeMethod = keyof SageBridgeRequestParamsMap;

type SageBridgeRequestBase<M extends SageBridgeMethod> = {
  channel: 'sage-bridge';
  bridgeVersion?: SageBridgeVersion;
  id: string;
  method: M;
};

export type SageBridgeRequestForMethod<M extends SageBridgeMethod> =
  SageBridgeRequestParamsMap[M] extends undefined
    ? SageBridgeRequestBase<M>
    : SageBridgeRequestBase<M> & {
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

export interface BridgeMethodDefinition<M extends SageBridgeMethod> {
  permission?: BridgePermissionPolicy;
  approval?: BridgeApprovalPolicy<M>;
  handle: (args: BridgeMethodHandlerArgs<M>) => Promise<unknown>;
}

export type BridgeMethodRegistry = Partial<{
  [M in SageBridgeMethod]: BridgeMethodDefinition<M>;
}>;
