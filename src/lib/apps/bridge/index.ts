import { runApprovalIfNeeded, BridgeApprovalDeniedError } from './approval';
import { enforcePermissionPolicy, BridgePermissionError } from './permissions';
import { bridgeMethods } from './registry';
import { failure, success } from './responses';
import type {
  BridgeMethodDefinition,
  SageBridgeContext,
  SageBridgeHostTools,
  SageBridgeMethod,
  SageBridgeRequest,
  SageBridgeResponse,
} from './types';

export type {
  BridgeApprovalRequest,
  BridgeApprovalResult,
  BridgeMethodDefinition,
  BridgeMethodRegistry,
  BridgePermissionPolicy,
  SageBridgeContext,
  SageBridgeErrorResponse,
  SageBridgeEventPayload,
  SageBridgeHostTools,
  SageBridgeMethod,
  SageBridgeRequest,
  SageBridgeRequestForMethod,
  SageBridgeRequestParamsMap,
  SageBridgeResponse,
  SageBridgeSendPayload,
  SageBridgeSuccessResponse,
  SageWalletSendXchRequest,
} from './types';

const KNOWN_BRIDGE_METHODS = new Set<string>(Object.keys(bridgeMethods));

export function isBridgeRequest(value: unknown): value is SageBridgeRequest {
  if (!value || typeof value !== 'object') {
    return false;
  }

  const maybe = value as {
    channel?: unknown;
    id?: unknown;
    method?: unknown;
  };

  return (
    maybe.channel === 'sage-bridge' &&
    typeof maybe.id === 'string' &&
    typeof maybe.method === 'string' &&
    KNOWN_BRIDGE_METHODS.has(maybe.method)
  );
}

export async function handleBridgeRequest(
  ctx: SageBridgeContext,
  request: SageBridgeRequest,
  tools: SageBridgeHostTools,
): Promise<SageBridgeResponse> {
  try {
    const definition = bridgeMethods[request.method as SageBridgeMethod] as
      | BridgeMethodDefinition<SageBridgeMethod>
      | undefined;

    if (!definition) {
      return failure(
        request.id,
        'method_not_found',
        `Unknown bridge method: ${request.method}`,
      );
    }

    await enforcePermissionPolicy({
      ctx,
      request,
      policy: definition.permission,
    });

    const approvalRequest = definition.approval
      ? await definition.approval({
          ctx,
          request: request as never,
        })
      : null;

    await runApprovalIfNeeded({
      approvalRequest,
      tools,
    });

    const result = await definition.handle({
      ctx,
      request: request as never,
      tools,
    });

    return success(request.id, result);
  } catch (error) {
    if (error instanceof BridgePermissionError) {
      return failure(request.id, 'permission_denied', error.message);
    }

    if (error instanceof BridgeApprovalDeniedError) {
      return failure(request.id, 'user_denied', error.message);
    }

    const message =
      error instanceof Error ? error.message : 'Unknown Sage bridge error';

    return failure(request.id, 'internal_error', message);
  }
}
