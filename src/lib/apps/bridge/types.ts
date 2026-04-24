import type { RustBridgeApprovalEvent } from '@/bindings';

export type BridgeApprovalRequest = RustBridgeApprovalEvent['approval'] & {
  approvalId: string;
};

export type BridgeApprovalResult =
  | { approved: true }
  | { approved: false; reason?: string };
