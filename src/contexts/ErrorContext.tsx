import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { reconcileActiveKeyProtection } from '@/state';
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

// Must match `CANCELLED_REASON` in crates/sage-password-gate/src/resolve.rs.
// That constant is returned with ErrorKind::Unauthorized when the user
// dismisses the password prompt; if the two strings drift, the cancel toast
// silently comes back.
const PASSWORD_CANCELLED_REASON = 'Password entry cancelled';

// Must match the reason returned at crates/sage-password-gate/src/resolve.rs:64
// when the user exhausts MAX_ATTEMPTS incorrect password attempts.
const PASSWORD_TOO_MANY_ATTEMPTS_REASON =
  'Too many incorrect password attempts';

// Must match the reason returned at crates/sage-password-gate/src/lib.rs:92
// when the password prompt is not answered within PROMPT_TIMEOUT.
const PASSWORD_PROMPT_TIMED_OUT_REASON = 'Password prompt timed out';

export function ErrorProvider({ children }: { children: ReactNode }) {
  const [errors, setErrors] = useState<CustomError[]>([]);

  const addError = useCallback((error: CustomError) => {
    if (
      error.kind === 'unauthorized' &&
      error.reason === PASSWORD_CANCELLED_REASON
    ) {
      // Deliberate user cancellation of the password prompt, not a failure.
      return;
    }
    if (error.kind === 'incorrect_password') {
      // Wrong password — AES decryption failed
      toast.error(t`Incorrect password`);
      // Self-heal if the active wallet's has_password flag drifted false:
      // this corrects it so the next attempt prompts for the password.
      void reconcileActiveKeyProtection();
      return;
    }
    if (error.kind === 'unauthorized') {
      const reason = error.reason ?? '';
      if (
        reason.includes('not found') ||
        reason.includes('No secret') ||
        reason === PASSWORD_TOO_MANY_ATTEMPTS_REASON ||
        reason === PASSWORD_PROMPT_TIMED_OUT_REASON
      ) {
        // KeyNotFound / NoSecretKey (wallet-level issue, not a transition),
        // or a genuine password-gate failure (too many attempts / timeout).
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
