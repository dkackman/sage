import { invoke } from '@tauri-apps/api/core';
import type {
  InstalledSageApp,
  SageBridgeFetchBatchRequest,
  SageBridgeFetchRequest,
  SageBridgeFetchResponse,
} from '@/bindings';

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

export type SageBridgeResponse =
  | SageBridgeSuccessResponse
  | SageBridgeErrorResponse;

export interface SageBridgeWebSocketOpenRequest {
  url: string;
  protocols?: string[];
}

export interface SageBridgeWebSocketSendRequest {
  socketId: string;
  text?: string;
  base64?: string;
}

export interface SageBridgeWebSocketCloseRequest {
  socketId: string;
  code?: number;
  reason?: string;
}

export interface SageBridgeWebSocketEvent {
  socketId: string;
  type: 'open' | 'message' | 'close' | 'error';
  data?:
    | {
        kind: 'text';
        text: string;
      }
    | {
        kind: 'binaryBase64';
        base64: string;
      };
  code?: number;
  reason?: string;
  wasClean?: boolean;
  message?: string;
}

export interface SageBridgeEventEnvelope {
  channel: 'sage-bridge';
  event: 'network.websocket';
  payload: SageBridgeWebSocketEvent;
}

export interface SageBridgeContext {
  app: InstalledSageApp;
  sourceLabel: string;
  emitEvent?: (event: SageBridgeEventEnvelope) => Promise<void> | void;
}

export interface SageBridgeEventPayload {
  sourceLabel: string;
  appId: string;
  request: SageBridgeRequest;
}

function success(id: string, result: unknown): SageBridgeSuccessResponse {
  return {
    channel: 'sage-bridge',
    id,
    ok: true,
    result,
  };
}

function failure(
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

export function isBridgeRequest(value: unknown): value is SageBridgeRequest {
  if (!value || typeof value !== 'object') {
    return false;
  }

  const maybe = value as Partial<SageBridgeRequest>;

  return (
    maybe.channel === 'sage-bridge' &&
    typeof maybe.id === 'string' &&
    typeof maybe.method === 'string'
  );
}

const appSockets = new Map<string, Map<string, WebSocket>>();

function randomId(prefix: string): string {
  return `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

function normalizeScheme(value: string): string {
  return value.trim().toLowerCase();
}

function normalizeHost(value: string): string {
  return value.trim().toLowerCase();
}

function hostMatchesPattern(host: string, pattern: string): boolean {
  const normalizedHost = normalizeHost(host);
  const normalizedPattern = normalizeHost(pattern);

  if (normalizedPattern === '*') {
    return true;
  }

  if (normalizedPattern.startsWith('*.')) {
    const suffix = normalizedPattern.slice(2);
    return normalizedHost.endsWith(`.${suffix}`);
  }

  return normalizedHost === normalizedPattern;
}

function isNetworkAllowedForApp(
  app: InstalledSageApp,
  scheme: string,
  host: string,
): boolean {
  const normalizedScheme = normalizeScheme(scheme);
  const normalizedHost = normalizeHost(host);

  return (app.grantedPermissions.network ?? []).some((entry) => {
    return (
      normalizeScheme(entry.scheme) === normalizedScheme &&
      hostMatchesPattern(normalizedHost, entry.host)
    );
  });
}

function getSocketMap(appId: string): Map<string, WebSocket> {
  let sockets = appSockets.get(appId);

  if (!sockets) {
    sockets = new Map<string, WebSocket>();
    appSockets.set(appId, sockets);
  }

  return sockets;
}

function getSocket(appId: string, socketId: string): WebSocket | undefined {
  return appSockets.get(appId)?.get(socketId);
}

function deleteSocket(appId: string, socketId: string) {
  const sockets = appSockets.get(appId);
  if (!sockets) {
    return;
  }

  sockets.delete(socketId);

  if (sockets.size === 0) {
    appSockets.delete(appId);
  }
}

function arrayBufferToBase64(buffer: ArrayBuffer): string {
  const bytes = new Uint8Array(buffer);
  let binary = '';

  for (let i = 0; i < bytes.length; i += 1) {
    binary += String.fromCharCode(bytes[i]);
  }

  return btoa(binary);
}

function base64ToUint8Array(base64: string): Uint8Array {
  const binary = atob(base64);
  const out = new Uint8Array(binary.length);

  for (let i = 0; i < binary.length; i += 1) {
    out[i] = binary.charCodeAt(i);
  }

  return out;
}

async function emitWebSocketEvent(
  ctx: SageBridgeContext,
  event: SageBridgeWebSocketEvent,
) {
  await ctx.emitEvent?.({
    channel: 'sage-bridge',
    event: 'network.websocket',
    payload: event,
  });
}

function cleanupSocketMap(appId: string) {
  const sockets = appSockets.get(appId);
  if (!sockets) {
    return;
  }

  for (const [, socket] of sockets) {
    try {
      socket.close();
    } catch {
      // ignore
    }
  }

  appSockets.delete(appId);
}

export function cleanupBridgeResources(appId: string) {
  cleanupSocketMap(appId);
}

async function handleWebSocketOpen(
  ctx: SageBridgeContext,
  requestId: string,
  params: SageBridgeWebSocketOpenRequest,
): Promise<SageBridgeResponse> {
  if (!params || typeof params.url !== 'string' || params.url.trim() === '') {
    return failure(requestId, 'invalid_params', 'Missing WebSocket URL');
  }

  let parsedUrl: URL;

  try {
    parsedUrl = new URL(params.url);
  } catch {
    return failure(requestId, 'invalid_params', 'Invalid WebSocket URL');
  }

  const scheme = normalizeScheme(parsedUrl.protocol.replace(/:$/, ''));
  const host = normalizeHost(parsedUrl.hostname);

  if (scheme !== 'ws' && scheme !== 'wss') {
    return failure(
      requestId,
      'invalid_params',
      `Unsupported WebSocket scheme: ${scheme}`,
    );
  }

  if (!isNetworkAllowedForApp(ctx.app, scheme, host)) {
    return failure(
      requestId,
      'network_access_denied',
      `Network access denied for ${scheme}://${host}`,
    );
  }

  const socketId = randomId('ws');
  const socketMap = getSocketMap(ctx.app.id);

  try {
    const socket =
      Array.isArray(params.protocols) && params.protocols.length > 0
        ? new WebSocket(parsedUrl.toString(), params.protocols)
        : new WebSocket(parsedUrl.toString());

    socket.binaryType = 'arraybuffer';
    socketMap.set(socketId, socket);

    socket.addEventListener('open', () => {
      void emitWebSocketEvent(ctx, {
        socketId,
        type: 'open',
      });
    });

    socket.addEventListener('message', (event) => {
      const data = event.data;

      if (typeof data === 'string') {
        void emitWebSocketEvent(ctx, {
          socketId,
          type: 'message',
          data: {
            kind: 'text',
            text: data,
          },
        });
        return;
      }

      if (data instanceof ArrayBuffer) {
        void emitWebSocketEvent(ctx, {
          socketId,
          type: 'message',
          data: {
            kind: 'binaryBase64',
            base64: arrayBufferToBase64(data),
          },
        });
        return;
      }

      if (data instanceof Blob) {
        void data.arrayBuffer().then((buffer) => {
          void emitWebSocketEvent(ctx, {
            socketId,
            type: 'message',
            data: {
              kind: 'binaryBase64',
              base64: arrayBufferToBase64(buffer),
            },
          });
        });
        return;
      }

      void emitWebSocketEvent(ctx, {
        socketId,
        type: 'error',
        message: 'Received unsupported WebSocket message type',
      });
    });

    socket.addEventListener('close', (event) => {
      deleteSocket(ctx.app.id, socketId);

      void emitWebSocketEvent(ctx, {
        socketId,
        type: 'close',
        code: event.code,
        reason: event.reason,
        wasClean: event.wasClean,
      });
    });

    socket.addEventListener('error', () => {
      void emitWebSocketEvent(ctx, {
        socketId,
        type: 'error',
        message: 'WebSocket error',
      });
    });

    return success(requestId, {
      socketId,
      readyState: socket.readyState,
    });
  } catch (error) {
    deleteSocket(ctx.app.id, socketId);

    return failure(
      requestId,
      'internal_error',
      error instanceof Error ? error.message : 'Failed to open WebSocket',
    );
  }
}

async function handleWebSocketSend(
  ctx: SageBridgeContext,
  requestId: string,
  params: SageBridgeWebSocketSendRequest,
): Promise<SageBridgeResponse> {
  if (
    !params ||
    typeof params.socketId !== 'string' ||
    params.socketId === ''
  ) {
    return failure(requestId, 'invalid_params', 'Missing socketId');
  }

  const socket = getSocket(ctx.app.id, params.socketId);

  if (!socket) {
    return failure(requestId, 'not_found', 'WebSocket not found');
  }

  if (socket.readyState !== WebSocket.OPEN) {
    return failure(
      requestId,
      'invalid_state',
      `WebSocket is not open (state ${socket.readyState})`,
    );
  }

  try {
    if (typeof params.text === 'string') {
      socket.send(params.text);
    } else if (typeof params.base64 === 'string') {
      socket.send(base64ToUint8Array(params.base64));
    } else {
      return failure(
        requestId,
        'invalid_params',
        'Provide either text or base64 payload',
      );
    }

    return success(requestId, { ok: true });
  } catch (error) {
    return failure(
      requestId,
      'internal_error',
      error instanceof Error ? error.message : 'Failed to send WebSocket data',
    );
  }
}

async function handleWebSocketClose(
  ctx: SageBridgeContext,
  requestId: string,
  params: SageBridgeWebSocketCloseRequest,
): Promise<SageBridgeResponse> {
  if (
    !params ||
    typeof params.socketId !== 'string' ||
    params.socketId === ''
  ) {
    return failure(requestId, 'invalid_params', 'Missing socketId');
  }

  const socket = getSocket(ctx.app.id, params.socketId);

  if (!socket) {
    return failure(requestId, 'not_found', 'WebSocket not found');
  }

  try {
    socket.close(params.code, params.reason);
    return success(requestId, { ok: true });
  } catch (error) {
    return failure(
      requestId,
      'internal_error',
      error instanceof Error ? error.message : 'Failed to close WebSocket',
    );
  }
}

export async function handleBridgeRequest(
  ctx: SageBridgeContext,
  request: SageBridgeRequest,
): Promise<SageBridgeResponse> {
  try {
    switch (request.method) {
      case 'bridge.ping':
        return success(request.id, {
          ok: true,
          appId: ctx.app.id,
          appName: ctx.app.name,
        });

      case 'app.getInfo':
        return success(request.id, {
          id: ctx.app.id,
          name: ctx.app.name,
          version: ctx.app.version,
        });

      case 'sage.getPermissions':
        return success(request.id, ctx.app.grantedPermissions);

      case 'network.fetch': {
        const params = request.params as SageBridgeFetchRequest;

        const response = await invoke<SageBridgeFetchResponse>(
          'bridge_fetch_http',
          {
            appId: ctx.app.id,
            req: params,
          },
        );

        return success(request.id, response);
      }

      case 'network.fetchBatch': {
        const params = request.params as SageBridgeFetchBatchRequest;

        const response = await invoke<SageBridgeFetchResponse[]>(
          'bridge_fetch_http_batch',
          {
            appId: ctx.app.id,
            req: params,
          },
        );

        return success(request.id, response);
      }

      case 'network.fetchBatchStream': {
        const params = request.params as SageBridgeFetchBatchRequest;

        const batchId = await invoke<string>('bridge_fetch_http_batch_stream', {
          appId: ctx.app.id,
          sourceLabel: ctx.sourceLabel,
          req: params,
        });

        return success(request.id, batchId);
      }

      case 'network.wsOpen':
        return handleWebSocketOpen(
          ctx,
          request.id,
          request.params as SageBridgeWebSocketOpenRequest,
        );

      case 'network.wsSend':
        return handleWebSocketSend(
          ctx,
          request.id,
          request.params as SageBridgeWebSocketSendRequest,
        );

      case 'network.wsClose':
        return handleWebSocketClose(
          ctx,
          request.id,
          request.params as SageBridgeWebSocketCloseRequest,
        );

      default:
        return failure(
          request.id,
          'method_not_found',
          `Unknown bridge method: ${request.method}`,
        );
    }
  } catch (error) {
    const message =
      error instanceof Error ? error.message : 'Unknown Sage bridge error';

    return failure(request.id, 'internal_error', message);
  }
}
