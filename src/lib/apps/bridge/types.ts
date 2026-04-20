import type { InstalledSageApp } from '@/bindings';

export type {
  SageBridgeVersion,
  SageBridgeRequest,
  SageBridgeResponse,
  SageBridgeSuccessResponse,
  SageBridgeErrorResponse,
  SageBridgeSendPayload,
  SageWalletSendXchRequest,
  SageRequestedNetworkWhitelistEntry,
  SageGrantedCapabilitiesChangeEvent,
  SageGrantedNetworkWhitelistChangeEvent,
  SageRequestCapabilityGrantInput,
  SageRequestCapabilityGrantResult,
  SageRequestNetworkWhitelistGrantInput,
  SageRequestNetworkWhitelistGrantResult,
} from '@sage-app/sdk';

import type {
  SageBridgeRequest,
  SageBridgeSendPayload,
  SageRequestedNetworkWhitelistEntry,
  SageRequestCapabilityGrantInput,
  SageRequestCapabilityGrantResult,
  SageRequestNetworkWhitelistGrantInput,
  SageRequestNetworkWhitelistGrantResult,
  SageWalletSendXchRequest,
} from '@sage-app/sdk';

export type SageBridgeMethod =
  | 'bridge.ping'
  | 'bridge.send'
  | 'app.getInfo'
  | 'sage.getCapabilities'
  | 'sage.requestCapabilityGrant'
  | 'sage.requestNetworkWhitelistGrant'
  | 'sage.requestNetwortWhitelistGrant'
  | 'wallet.sendXch';

export interface SageBridgeRequestParamsMap {
  'bridge.ping': undefined;
  'bridge.send': SageBridgeSendPayload;
  'app.getInfo': undefined;
  'sage.getCapabilities': undefined;
  'sage.requestCapabilityGrant': SageRequestCapabilityGrantInput;
  'sage.requestNetworkWhitelistGrant': SageRequestNetworkWhitelistGrantInput;
  'sage.requestNetwortWhitelistGrant': SageRequestNetworkWhitelistGrantInput;
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

export type BridgeApprovalRequest =
  | {
      kind: 'send_xch';
      app: InstalledSageApp;
      sourceLabel: string;
      requestId: string;
      params: SageWalletSendXchRequest;
    }
  | {
      kind: 'capability_grant';
      app: InstalledSageApp;
      sourceLabel: string;
      requestId: string;
      capability: string;
    }
  | {
      kind: 'network_whitelist_grant';
      app: InstalledSageApp;
      sourceLabel: string;
      requestId: string;
      entry: SageRequestedNetworkWhitelistEntry;
    };

export type BridgeApprovalResult =
  | { approved: true }
  | { approved: false; reason?: string };

export function isKnownSageBridgeMethod(
  method: string,
): method is SageBridgeMethod {
  return (
    method === 'bridge.ping' ||
    method === 'bridge.send' ||
    method === 'app.getInfo' ||
    method === 'sage.getCapabilities' ||
    method === 'sage.requestCapabilityGrant' ||
    method === 'sage.requestNetworkWhitelistGrant' ||
    method === 'sage.requestNetwortWhitelistGrant' ||
    method === 'wallet.sendXch'
  );
}

export function isKnownSageBridgeRequest(
  request: SageBridgeRequest,
): request is KnownSageBridgeRequest {
  return isKnownSageBridgeMethod(request.method);
}
