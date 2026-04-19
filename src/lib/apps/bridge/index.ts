import type { SageBridgeRequest, SageBridgeVersion } from './types';
import { isKnownSageBridgeMethod } from './types';

export type {
  BridgeApprovalRequest,
  BridgeApprovalResult,
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
    isKnownSageBridgeMethod(maybe.method)
  );
}
