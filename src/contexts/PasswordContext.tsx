import { PasswordDialog } from '@/components/dialogs/PasswordDialog';
import { useBiometric } from '@/hooks/useBiometric';
import { commands, events, PasswordRequest } from '@/bindings';
import { platform } from '@tauri-apps/plugin-os';
import {
  createContext,
  ReactNode,
  useCallback,
  useEffect,
  useRef,
  useState,
} from 'react';

const isMobile = platform() === 'ios' || platform() === 'android';

// Biometric caching interval (5 minutes)
const BIOMETRIC_CACHE_MS = 5 * 60 * 1000;

export interface PasswordContextType {
  /**
   * UI-only authentication gate for actions that touch no wallet secret
   * (starting the RPC server, toggling run-on-startup). Returns true if the
   * caller may proceed. This deliberately does NOT go through the Rust
   * password gate — there is no unlock operation behind it.
   */
  requireLocalAuth: () => Promise<boolean>;
}

export const PasswordContext = createContext<PasswordContextType | undefined>(
  undefined,
);

export function PasswordProvider({ children }: { children: ReactNode }) {
  // A queue rather than a single slot: Rust can have multiple gated
  // operations in flight concurrently (one per requestId), and each one
  // needs a reply eventually or it hangs for the full 5-minute timeout.
  // The dialog always shows the front of the queue; later requests wait
  // their turn instead of clobbering the one in progress.
  const [queue, setQueue] = useState<PasswordRequest[]>([]);
  const pending = queue[0] ?? null;
  const { enabled: biometricEnabled } = useBiometric();
  const lastBiometricPromptRef = useRef<number | null>(null);

  const runBiometric = useCallback(async (): Promise<boolean> => {
    const now = performance.now();
    if (
      lastBiometricPromptRef.current !== null &&
      now - lastBiometricPromptRef.current < BIOMETRIC_CACHE_MS
    ) {
      return true;
    }
    try {
      const { authenticate } = await import('@tauri-apps/plugin-biometric');
      await authenticate('Authenticate to continue', {
        allowDeviceCredential: false,
      });
      lastBiometricPromptRef.current = now;
      return true;
    } catch {
      return false;
    }
  }, []);

  const requireLocalAuth = useCallback(async (): Promise<boolean> => {
    if (!biometricEnabled || !isMobile) return true;
    return runBiometric();
  }, [biometricEnabled, runBiometric]);

  useEffect(() => {
    const unlisten = events.passwordRequest.listen(async ({ payload }) => {
      // Case 1: password takes precedence — enqueue for the dialog. If a
      // request with the same requestId is already queued (a retry after a
      // wrong password), replace it in place rather than duplicating it.
      if (payload.requiresPassword) {
        setQueue((prev) => {
          const index = prev.findIndex(
            (r) => r.requestId === payload.requestId,
          );
          if (index === -1) return [...prev, payload];
          const next = [...prev];
          next[index] = payload;
          return next;
        });
        return;
      }

      // Case 2: no password, biometric enabled — standalone gate with cache.
      if (biometricEnabled && isMobile) {
        const ok = await runBiometric();
        await commands.submitPasswordResponse(
          payload.requestId,
          ok ? { kind: 'no_auth_needed' } : { kind: 'cancelled' },
        );
        return;
      }

      // Case 3: no password, no biometric — nothing to do.
      await commands.submitPasswordResponse(payload.requestId, {
        kind: 'no_auth_needed',
      });
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [biometricEnabled, runBiometric]);

  const handleSubmit = useCallback(
    (password: string) => {
      if (!pending) return;
      setQueue((prev) => prev.slice(1));
      commands.submitPasswordResponse(pending.requestId, {
        kind: 'password',
        password,
      });
    },
    [pending],
  );

  const handleCancel = useCallback(() => {
    if (!pending) return;
    setQueue((prev) => prev.slice(1));
    commands.submitPasswordResponse(pending.requestId, { kind: 'cancelled' });
  }, [pending]);

  return (
    <PasswordContext.Provider value={{ requireLocalAuth }}>
      {children}
      <PasswordDialog
        open={pending !== null}
        attemptsRemaining={pending?.error?.attemptsRemaining ?? undefined}
        onSubmit={handleSubmit}
        onCancel={handleCancel}
      />
    </PasswordContext.Provider>
  );
}
