export { initSageRuntimeBridge, SAGE_BRIDGE_VERSION } from './runtime';

export {
    isSageRuntimeAvailable,
    isSageBridgeInitialized,
    createSageClient,
    formatSageError,
    getSageClientSync,
    hasSageBridge,
} from './client';

export type {
    Amount,
    SageAppInfo,
    SageBridgeErrorResponse,
    SageBridgeEventPayload,
    SageBridgeRequest,
    SageBridgeResponse,
    SageBridgeSendPayload,
    SageBridgeSuccessResponse,
    SageBridgeVersion,
    SageClient,
    SageRequestedPermissions,
    SageLifecycleBeforeStopDetail,
    SageNetworkPermission,
    SageWalletSendXchRequest,
    TransactionResponse,
} from './types';
