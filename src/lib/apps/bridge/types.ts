import type { SystemSageApp, UserSageApp } from '@/bindings';
import type {
  RequestCapabilityGrantParams,
  RequestNetworkWhitelistGrantParams,
  WalletSendXchParams,
} from '@sage-app/sdk';

export type ApprovalApp =
  | ({ kind: 'user' } & UserSageApp)
  | ({ kind: 'system' } & SystemSageApp);

export type BridgeApprovalRequest =
  | {
      kind: 'send_xch';
      app: ApprovalApp;
      sourceLabel: string;
      requestId: string;
      params: WalletSendXchParams;
    }
  | {
      kind: 'capability_grant';
      app: ApprovalApp;
      sourceLabel: string;
      requestId: string;
      params: RequestCapabilityGrantParams;
    }
  | {
      kind: 'network_whitelist_grant';
      app: ApprovalApp;
      sourceLabel: string;
      requestId: string;
      params: RequestNetworkWhitelistGrantParams;
    };

export type BridgeApprovalResult =
  | { approved: true }
  | { approved: false; reason?: string };
