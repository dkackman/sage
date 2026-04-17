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
  SageBridgeVersion,
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
  SageBridgeVersion,
  SageWalletSendXchRequest,
} from './types';

const KNOWN_BRIDGE_METHODS = new Set<string>(Object.keys(bridgeMethods));
const SUPPORTED_BRIDGE_VERSION: SageBridgeVersion = 'v1';

export function isSupportedBridgeVersion(
  value: unknown,
): value is SageBridgeVersion {
  return value === SUPPORTED_BRIDGE_VERSION;
}

export function isBridgeRequest(value: unknown): value is SageBridgeRequest {
  if (!value || typeof value !== 'object') {
    return false;
  }

  const maybe = value as {
    channel?: unknown;
    bridgeVersion?: unknown;
    id?: unknown;
    method?: unknown;
  };

  if (maybe.channel !== 'sage-bridge') {
    return false;
  }

  if (
    maybe.bridgeVersion !== undefined &&
    !isSupportedBridgeVersion(maybe.bridgeVersion)
  ) {
    return false;
  }

  return (
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
    if (
      request.bridgeVersion !== undefined &&
      request.bridgeVersion !== SUPPORTED_BRIDGE_VERSION
    ) {
      return failure(
        request.id,
        'unsupported_bridge_version',
        `Unsupported Sage bridge version: ${String(request.bridgeVersion)}`,
      );
    }

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
