import type {
  BridgePermissionPolicy,
  SageBridgeContext,
  SageBridgeMethod,
  SageBridgeRequest,
} from './types';

export class BridgePermissionError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'BridgePermissionError';
  }
}

function permissionKeyForMethod(method: SageBridgeMethod): string {
  switch (method) {
    case 'wallet.sendXch':
      return 'wallet.send_xch';

    default:
      return method;
  }
}

async function enforcePermissionByMethod(
  ctx: SageBridgeContext,
  method: SageBridgeMethod,
): Promise<void> {
  const permissionKey = permissionKeyForMethod(method);

  if (!ctx.app.grantedPermissions.includes(permissionKey)) {
    throw new BridgePermissionError(`Permission denied for ${permissionKey}`);
  }
}

export async function enforcePermissionPolicy(args: {
  ctx: SageBridgeContext;
  request: SageBridgeRequest;
  policy?: BridgePermissionPolicy;
}): Promise<void> {
  const { ctx, request, policy } = args;

  if (!policy) {
    await enforcePermissionByMethod(ctx, request.method);
    return;
  }

  switch (policy.kind) {
    case 'none':
      return;

    case 'custom':
      await policy.check({ ctx, request });
      return;
  }
}
