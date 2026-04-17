import type {
  SageBridgeErrorResponse,
  SageBridgeSuccessResponse,
  SageBridgeVersion,
} from './types';

const BRIDGE_VERSION: SageBridgeVersion = 'v1';

export function success(
  id: string,
  result: unknown,
): SageBridgeSuccessResponse {
  return {
    channel: 'sage-bridge',
    bridgeVersion: BRIDGE_VERSION,
    id,
    ok: true,
    result,
  };
}

export function failure(
  id: string,
  code: string,
  message: string,
): SageBridgeErrorResponse {
  return {
    channel: 'sage-bridge',
    bridgeVersion: BRIDGE_VERSION,
    id,
    ok: false,
    error: {
      code,
      message,
    },
  };
}
