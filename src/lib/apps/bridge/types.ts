import type { SystemSageApp, UserSageApp } from '@/bindings';

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
  SageRequestedNetworkWhitelistEntry,
  SageWalletSendXchRequest,
} from '@sage-app/sdk';

export type ApprovalApp =
  | ({ kind: 'user' } & UserSageApp)
  | ({ kind: 'system' } & SystemSageApp);

export type SageBridgeMethod =
  | 'bridge.ping'
  | 'bridge.send'
  | 'app.getInfo'
  | 'sage.getCapabilities'
  | 'sage.requestCapabilityGrant'
  | 'sage.requestNetworkWhitelistGrant'
  | 'sage.requestNetwortWhitelistGrant'
  | 'wallet.sendXch';

export type BridgeApprovalRequest =
  | {
      kind: 'send_xch';
      app: ApprovalApp;
      sourceLabel: string;
      requestId: string;
      params: SageWalletSendXchRequest;
    }
  | {
      kind: 'capability_grant';
      app: ApprovalApp;
      sourceLabel: string;
      requestId: string;
      capability: string;
    }
  | {
      kind: 'network_whitelist_grant';
      app: ApprovalApp;
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
