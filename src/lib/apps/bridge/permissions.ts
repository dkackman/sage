import type {
  BridgePermissionPolicy,
  KnownSageBridgeRequest,
  SageBridgeContext,
  SageBridgeMethod,
} from './types';

export class BridgePermissionError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'BridgePermissionError';
  }
}

function camelToSnakeSegment(segment: string): string {
  return segment.replace(/[A-Z]/g, (ch) => `_${ch.toLowerCase()}`);
}

export function bridgeMethodToPermissionKey(method: string): string {
  const [group, action] = method.split('.');
  if (!group || !action) {
    throw new Error(`Invalid bridge method: ${method}`);
  }

  return `${group}.${camelToSnakeSegment(action)}`;
}

async function enforcePermissionByMethod(
  ctx: SageBridgeContext,
  method: SageBridgeMethod,
): Promise<void> {
  const permissionKey = bridgeMethodToPermissionKey(method);

  if (!ctx.app.grantedPermissions.capabilities.includes(permissionKey)) {
    throw new BridgePermissionError(`Permission denied for ${permissionKey}`);
  }
}

export async function enforcePermissionPolicy(args: {
  ctx: SageBridgeContext;
  request: KnownSageBridgeRequest;
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
