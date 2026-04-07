import { PasswordDialog } from '@/components/dialogs/PasswordDialog';
import { useBiometric } from '@/hooks/useBiometric';
import { usePasskey } from '@/hooks/usePasskey';
import { platform } from '@tauri-apps/plugin-os';
import { createContext, ReactNode, useCallback, useRef, useState } from 'react';

const isMobile = platform() === 'ios' || platform() === 'android';

// Biometric caching interval (5 minutes)
const BIOMETRIC_CACHE_MS = 5 * 60 * 1000;

interface PasswordRequest {
  resolve: (password: string | null | undefined) => void;
}

/**
 * Auth credential returned by requestAuth.
 * Callers can spread this directly into API request objects.
 */
export interface AuthCredential {
  password?: string | null;
  prf_output?: string | null;
}

/** Info about the wallet's protection method. */
export interface WalletAuth {
  has_password: boolean;
  has_passkey: boolean;
  credential_id?: string | null;
  prf_salt?: string | null;
}

export interface PasswordContextType {
  /** @deprecated Use requestAuth instead. */
  requestPassword: (hasPassword: boolean) => Promise<string | null | undefined>;
  /**
   * Request authentication for a wallet.
   * Returns AuthCredential on success, undefined if cancelled.
   */
  requestAuth: (wallet: WalletAuth) => Promise<AuthCredential | undefined>;
}

export const PasswordContext = createContext<PasswordContextType | undefined>(
  undefined,
);

export function PasswordProvider({ children }: { children: ReactNode }) {
  const [open, setOpen] = useState(false);
  const pendingRef = useRef<PasswordRequest | null>(null);
  const { enabled: biometricEnabled } = useBiometric();
  const { authenticatePasskey } = usePasskey();

  // Biometric caching for standalone gate
  const lastBiometricPromptRef = useRef<number | null>(null);

  const requestPassword = useCallback(
    async (hasPassword: boolean): Promise<string | null | undefined> => {
      // Case 1: Has password → password takes precedence, show dialog
      if (hasPassword) {
        return new Promise<string | null | undefined>((resolve) => {
          pendingRef.current = { resolve };
          setOpen(true);
        });
      }

      // Case 2: No password, biometric enabled → standalone biometric gate with 5-min cache
      if (biometricEnabled && isMobile) {
        const now = performance.now();
        if (
          lastBiometricPromptRef.current !== null &&
          now - lastBiometricPromptRef.current < BIOMETRIC_CACHE_MS
        ) {
          return null; // Within cache window, skip prompt
        }

        try {
          const { authenticate } = await import('@tauri-apps/plugin-biometric');
          await authenticate('Authenticate to continue', {
            allowDeviceCredential: false,
          });
          lastBiometricPromptRef.current = now;
          return null;
        } catch {
          return undefined; // biometric failed/cancelled
        }
      }

      // Case 3: No password, no biometric → no auth needed
      return null;
    },
    [biometricEnabled],
  );

  const requestAuth = useCallback(
    async (wallet: WalletAuth): Promise<AuthCredential | undefined> => {
      // Case 1: Passkey-protected → trigger WebAuthn PRF ceremony
      if (wallet.has_passkey && wallet.credential_id && wallet.prf_salt) {
        const prfHex = await authenticatePasskey(
          wallet.credential_id,
          wallet.prf_salt,
        );
        if (!prfHex) return undefined;
        return { prf_output: prfHex };
      }

      // Case 2: Password-protected → show password dialog
      const password = await requestPassword(wallet.has_password);
      if (password === undefined) return undefined;
      return { password };
    },
    [requestPassword],
  );

  const handleSubmit = useCallback((password: string) => {
    setOpen(false);
    pendingRef.current?.resolve(password);
    pendingRef.current = null;
  }, []);

  const handleCancel = useCallback(() => {
    setOpen(false);
    pendingRef.current?.resolve(undefined);
    pendingRef.current = null;
  }, []);

  return (
    <PasswordContext.Provider value={{ requestPassword, requestAuth }}>
      {children}
      <PasswordDialog
        open={open}
        onSubmit={handleSubmit}
        onCancel={handleCancel}
      />
    </PasswordContext.Provider>
  );
}
