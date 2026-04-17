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

function snakeToCamelSegment(segment: string): string {
  return segment.replace(/_([a-z])/g, (_, ch: string) => ch.toUpperCase());
}

function camelToSnakeSegment(segment: string): string {
  return segment.replace(/[A-Z]/g, (ch) => `_${ch.toLowerCase()}`);
}

export function permissionKeyToBridgeMethod(permissionKey: string): string {
  const [group, action] = permissionKey.split('.');
  if (!group || !action) {
    throw new Error(`Invalid permission key: ${permissionKey}`);
  }

  return `${group}.${snakeToCamelSegment(action)}`;
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
