import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { BridgeApprovalRequest } from '@/lib/apps/bridge/types.ts';

const APPROVAL_TIMEOUT_MS = 30_000;

export interface PendingApprovalItem {
  id: string;
  createdAt: number;
  expiresAt: number;
  request: BridgeApprovalRequest;
}

interface ApprovalResult {
  approved: boolean;
  reason?: string;
}

export function useAppPendingApprovals() {
  const [pendingApprovals, setPendingApprovals] = useState<
    PendingApprovalItem[]
  >([]);
  const [currentTimestampMs, setCurrentTimestampMs] = useState(() =>
    Date.now(),
  );

  const approvalResolversRef = useRef<
    Map<string, (result: ApprovalResult) => void>
  >(new Map());

  const currentApproval = pendingApprovals[0] ?? null;
  const queuedApprovalCount = Math.max(0, pendingApprovals.length - 1);

  const currentApprovalSecondsLeft = useMemo(() => {
    if (!currentApproval) {
      return 0;
    }

    return Math.max(
      0,
      Math.ceil((currentApproval.expiresAt - currentTimestampMs) / 1000),
    );
  }, [currentApproval, currentTimestampMs]);

  const requestApproval = useCallback(
    async (approval: BridgeApprovalRequest): Promise<ApprovalResult> => {
      const now = Date.now();

      const item: PendingApprovalItem = {
        id: approval.requestId,
        createdAt: now,
        expiresAt: now + APPROVAL_TIMEOUT_MS,
        request: approval,
      };

      setPendingApprovals((prev) => [...prev, item]);

      return await new Promise((resolve) => {
        approvalResolversRef.current.set(item.id, resolve);
      });
    },
    [],
  );

  const resolveApprovalById = useCallback(
    (id: string, result: ApprovalResult) => {
      const resolver = approvalResolversRef.current.get(id);

      if (resolver) {
        approvalResolversRef.current.delete(id);
        resolver(result);
      }

      setPendingApprovals((prev) => prev.filter((item) => item.id !== id));
    },
    [],
  );

  const approveCurrentApproval = useCallback(() => {
    if (!currentApproval) {
      return;
    }

    resolveApprovalById(currentApproval.id, {
      approved: true,
    });
  }, [currentApproval, resolveApprovalById]);

  const rejectCurrentApproval = useCallback(() => {
    if (!currentApproval) {
      return;
    }

    resolveApprovalById(currentApproval.id, {
      approved: false,
      reason: 'User denied the request',
    });
  }, [currentApproval, resolveApprovalById]);

  useEffect(() => {
    if (!currentApproval) {
      return;
    }

    const intervalId = window.setInterval(() => {
      setCurrentTimestampMs(Date.now());
    }, 250);

    return () => {
      window.clearInterval(intervalId);
    };
  }, [currentApproval]);

  useEffect(() => {
    if (!currentApproval) {
      return;
    }

    const remainingMs = Math.max(0, currentApproval.expiresAt - Date.now());

    const timeoutId = window.setTimeout(() => {
      resolveApprovalById(currentApproval.id, {
        approved: false,
        reason: 'Approval request timed out',
      });
    }, remainingMs);

    return () => {
      window.clearTimeout(timeoutId);
    };
  }, [currentApproval, resolveApprovalById]);

  useEffect(() => {
    return () => {
      for (const [id, resolve] of approvalResolversRef.current.entries()) {
        resolve({
          approved: false,
          reason: 'App host unmounted',
        });
        approvalResolversRef.current.delete(id);
      }
    };
  }, []);

  return {
    currentApproval,
    queuedApprovalCount,
    currentApprovalSecondsLeft,
    requestApproval,
    approveCurrentApproval,
    rejectCurrentApproval,
  };
}
