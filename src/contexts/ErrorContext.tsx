import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { reconcileDriftedKeyProtection } from '@/state';
import { t } from '@lingui/core/macro';
import { createContext, ReactNode, useCallback, useState } from 'react';
import { toast } from 'react-toastify';
import { ErrorKind } from '../bindings';

export interface CustomError {
  kind: ErrorKind | 'walletconnect' | 'upload' | 'invalid' | 'dexie';
  reason: string;
}

export interface ErrorContextType {
  errors: CustomError[];
  addError: (error: CustomError) => void;
}

export const ErrorContext = createContext<ErrorContextType | undefined>(
  undefined,
);

export function ErrorProvider({ children }: { children: ReactNode }) {
  const [errors, setErrors] = useState<CustomError[]>([]);

  const addError = useCallback((error: CustomError) => {
    // The password-gate outcomes arrive as structured kinds (see
    // sage-api's ErrorKind), so they are matched on kind and rendered as
    // translated text; the English `reason` is purely human-facing.
    if (error.kind === 'password_cancelled') {
      // Deliberate user cancellation of the password prompt, not a failure.
      return;
    }
    if (error.kind === 'incorrect_password') {
      // Wrong password — AES decryption failed
      toast.error(t`Incorrect password`);
      // Self-heal if a wallet's has_password flag drifted false: this
      // corrects it so the next attempt prompts for the password. The sweep
      // covers every wallet, not just the active one, because delete_key and
      // get_secret_key gate on their own fingerprint from the wallet list.
      void reconcileDriftedKeyProtection();
      return;
    }
    if (error.kind === 'too_many_password_attempts') {
      toast.error(t`Too many incorrect password attempts`);
      return;
    }
    if (error.kind === 'password_prompt_timed_out') {
      toast.error(t`Password prompt timed out`);
      return;
    }
    if (error.kind === 'unauthorized') {
      const reason = error.reason ?? '';
      if (reason.includes('not found') || reason.includes('No secret')) {
        // KeyNotFound / NoSecretKey: a wallet-level issue, not a transition.
        toast.error(error.reason);
      }
      // NotLoggedIn / NoSigningKey during wallet transitions are silently ignored
      return;
    }
    setErrors((prevErrors) => [...prevErrors, error]);
  }, []);

  return (
    <ErrorContext.Provider value={{ errors, addError }}>
      {children}

      {errors.length > 0 && (
        <ErrorDialog
          error={errors[0]}
          setError={() => setErrors((prevErrors) => prevErrors.slice(1))}
        />
      )}
    </ErrorContext.Provider>
  );
}

export interface ErrorDialogProps {
  error: CustomError | null;
  setError: (error: CustomError | null) => void;
}

export default function ErrorDialog({ error, setError }: ErrorDialogProps) {
  let kind: string | null;

  switch (error?.kind) {
    case 'api':
      kind = 'API';
      break;

    case 'internal':
      kind = 'Internal';
      break;

    case 'not_found':
      kind = 'Not Found';
      break;

    case 'unauthorized':
      kind = 'Auth';
      break;

    case 'wallet':
      kind = 'Wallet';
      break;

    case 'walletconnect':
      kind = 'WalletConnect';
      break;

    case 'upload':
      kind = 'Upload';
      break;

    case 'nfc':
      kind = 'NFC';
      break;

    case 'database_migration':
      kind = 'Database Migration';
      break;

    case 'dexie':
      kind = 'Dexie';
      break;

    default:
      kind = null;
  }

  return (
    <Dialog open={error !== null} onOpenChange={() => setError(null)}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{kind ? `${kind} ` : ''}Error</DialogTitle>
          <DialogDescription className='break-words hyphens-auto'>
            {error?.reason}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button onClick={() => setError(null)} autoFocus>
            Ok
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
