import type { InstalledSageApp } from '@/bindings';

export type {
  SageBridgeVersion,
  SageBridgeRequest,
  SageBridgeResponse,
  SageBridgeSuccessResponse,
  SageBridgeErrorResponse,
  SageBridgeEventPayload,
  SageBridgeSendPayload,
  SageWalletSendXchRequest,
} from '@sage-app/sdk';

import type {
  SageBridgeRequest,
  SageBridgeSendPayload,
  SageWalletSendXchRequest,
} from '@sage-app/sdk';

export type SageBridgeMethod =
  | 'bridge.ping'
  | 'bridge.send'
  | 'app.getInfo'
  | 'sage.getCapabilities'
  | 'wallet.sendXch';

export interface SageBridgeRequestParamsMap {
  'bridge.ping': undefined;
  'bridge.send': SageBridgeSendPayload;
  'app.getInfo': undefined;
  'sage.getCapabilities': undefined;
  'wallet.sendXch': SageWalletSendXchRequest;
}

interface SageBridgeRequestBase<M extends SageBridgeMethod> {
  channel: 'sage-bridge';
  id: string;
  method: M;
  bridgeVersion: 'v1';
}

export type SageBridgeRequestForMethod<M extends SageBridgeMethod> =
  SageBridgeRequestParamsMap[M] extends undefined
    ? SageBridgeRequestBase<M>
    : SageBridgeRequestBase<M> & {
        params: SageBridgeRequestParamsMap[M];
      };

export type KnownSageBridgeRequest = {
  [M in SageBridgeMethod]: SageBridgeRequestForMethod<M>;
}[SageBridgeMethod];

export interface SageBridgeContext {
  app: InstalledSageApp;
  sourceLabel: string;
}

export type BridgePermissionPolicy =
  | { kind: 'none' }
  | {
      kind: 'custom';
      check: (args: {
        ctx: SageBridgeContext;
        request: KnownSageBridgeRequest;
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

export function isKnownSageBridgeMethod(
  method: string,
): method is SageBridgeMethod {
  return (
    method === 'bridge.ping' ||
    method === 'bridge.send' ||
    method === 'app.getInfo' ||
    method === 'sage.getCapabilities' ||
    method === 'wallet.sendXch'
  );
}

export function isKnownSageBridgeRequest(
  request: SageBridgeRequest,
): request is KnownSageBridgeRequest {
  return isKnownSageBridgeMethod(request.method);
}
