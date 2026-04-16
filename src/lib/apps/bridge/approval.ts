import type { BridgeApprovalRequest, SageBridgeHostTools } from './types';

export class BridgeApprovalDeniedError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'BridgeApprovalDeniedError';
  }
}

export async function runApprovalIfNeeded(args: {
  approvalRequest: BridgeApprovalRequest | null;
  tools: SageBridgeHostTools;
}): Promise<void> {
  const { approvalRequest, tools } = args;

  if (!approvalRequest) {
    return;
  }

  const result = await tools.requestApproval(approvalRequest);

  if (!result.approved) {
    throw new BridgeApprovalDeniedError(
      result.reason || 'User denied the request',
    );
  }
}
