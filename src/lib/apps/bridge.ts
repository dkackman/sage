export interface SageBridgeRequest {
  channel: 'sage-bridge';
  id: string;
  method: string;
  params?: unknown;
}

export interface SageBridgeSuccessResponse {
  channel: 'sage-bridge';
  id: string;
  ok: true;
  result: unknown;
}

export interface SageBridgeErrorResponse {
  channel: 'sage-bridge';
  id: string;
  ok: false;
  error: {
    code: string;
    message: string;
  };
}

export function isBridgeRequest(_value: unknown): _value is SageBridgeRequest {
  return false;
}

export function getAppOrigin(entry: string): string | null {
  try {
    return new URL(entry).origin;
  } catch {
    return null;
  }
}

export async function handleBridgeRequest(): Promise<
  SageBridgeSuccessResponse | SageBridgeErrorResponse
> {
  return {
    channel: 'sage-bridge',
    id: 'unsupported',
    ok: false,
    error: {
      code: 'unsupported',
      message: 'Bridge is not enabled for installed zip apps yet.',
    },
  };
}

