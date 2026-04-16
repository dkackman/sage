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

function getNestedValue(obj: unknown, path: string[]): unknown {
  let current: unknown = obj;

  for (const segment of path) {
    if (!current || typeof current !== 'object') {
      return undefined;
    }

    current = (current as Record<string, unknown>)[segment];
  }

  return current;
}

function methodToPermissionPath(method: SageBridgeMethod): string[] {
  const [group, action] = method.split('.');

  if (!group || !action) {
    throw new BridgePermissionError(
      `Cannot derive permission path from method: ${method}`,
    );
  }

  return [group, action];
}

async function enforceBooleanPermissionByMethod(
  ctx: SageBridgeContext,
  method: SageBridgeMethod,
): Promise<void> {
  const path = methodToPermissionPath(method);
  const value = getNestedValue(ctx.app.grantedPermissions, path);

  if (typeof value !== 'boolean') {
    throw new BridgePermissionError(
      `Permission ${path.join('.')} is not a boolean granted permission`,
    );
  }

  if (!value) {
    throw new BridgePermissionError(`Permission denied for ${path.join('.')}`);
  }
}

export async function enforcePermissionPolicy(args: {
  ctx: SageBridgeContext;
  request: SageBridgeRequest;
  policy?: BridgePermissionPolicy;
}): Promise<void> {
  const { ctx, request, policy } = args;

  if (!policy) {
    await enforceBooleanPermissionByMethod(ctx, request.method);
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
