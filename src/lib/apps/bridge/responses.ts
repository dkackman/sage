import type {
  SageBridgeErrorResponse,
  SageBridgeSuccessResponse,
} from './types';

export function success(
  id: string,
  result: unknown,
): SageBridgeSuccessResponse {
  return {
    channel: 'sage-bridge',
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
    id,
    ok: false,
    error: {
      code,
      message,
    },
  };
}
