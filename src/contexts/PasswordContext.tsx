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
  // BiometricContext owns the single biometric gate (and its 5-minute
  // cache); this context only decides *when* to invoke it.
  const { enabled: biometricEnabled, promptIfEnabled } = useBiometric();

  const requireLocalAuth = useCallback(async (): Promise<boolean> => {
    if (!isMobile) return true;
    return promptIfEnabled();
  }, [promptIfEnabled]);

  // The listener reads biometric state through a ref so it can be registered
  // exactly once. Re-registering on every `biometricEnabled` change would
  // open a gap between unlisten and the new (async) listen resolving, and a
  // PasswordRequest emitted in that gap would never be answered — the gated
  // operation would hang for the full prompt timeout.
  const biometricRef = useRef({ enabled: biometricEnabled, promptIfEnabled });
  biometricRef.current = { enabled: biometricEnabled, promptIfEnabled };

  useEffect(() => {
    const unlisten = events.passwordRequest.listen(async ({ payload }) => {
      // Case 1: password takes precedence — enqueue for the dialog. If a
      // request with the same requestId is already queued, replace it in
      // place rather than duplicating it. A wrong-password re-prompt arrives
      // *after* handleSubmit dequeued the entry, so it carries an `error`
      // payload but is no longer queued: put it back at the FRONT, resuming
      // the dialog the user was just typing into, rather than appending it
      // behind an unrelated request (where the user would mistake the next
      // request's prompt for this retry and type this password into it).
      if (payload.requiresPassword) {
        setQueue((prev) => {
          const index = prev.findIndex(
            (r) => r.requestId === payload.requestId,
          );
          if (index === -1) {
            return payload.error ? [payload, ...prev] : [...prev, payload];
          }
          const next = [...prev];
          next[index] = payload;
          return next;
        });
        return;
      }

      // Case 2: no password, biometric enabled — standalone gate with cache.
      if (isMobile && biometricRef.current.enabled) {
        const ok = await biometricRef.current.promptIfEnabled();
        await commands
          .submitPasswordResponse(
            payload.requestId,
            ok ? { kind: 'no_auth_needed' } : { kind: 'cancelled' },
          )
          .catch((error) => console.error('password response failed', error));
        return;
      }

      // Case 3: no password, no biometric — nothing to do.
      await commands
        .submitPasswordResponse(payload.requestId, { kind: 'no_auth_needed' })
        .catch((error) => console.error('password response failed', error));
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // A response can fail benignly: the Rust side times a request out after
  // PROMPT_TIMEOUT and clears it, so a submit into a stale dialog gets
  // NotFound. The operation's own timeout error already reaches the user via
  // ErrorContext; here it only must not become an unhandled rejection.
  const handleSubmit = useCallback(
    (password: string) => {
      if (!pending) return;
      setQueue((prev) => prev.slice(1));
      commands
        .submitPasswordResponse(pending.requestId, {
          kind: 'password',
          password,
        })
        .catch((error) => console.error('password response failed', error));
    },
    [pending],
  );

  const handleCancel = useCallback(() => {
    if (!pending) return;
    setQueue((prev) => prev.slice(1));
    commands
      .submitPasswordResponse(pending.requestId, { kind: 'cancelled' })
      .catch((error) => console.error('password response failed', error));
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
