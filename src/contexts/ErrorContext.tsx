import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import {
  createContext,
  ReactNode,
  useCallback,
  useEffect,
  useState,
} from 'react';
import { ErrorKind } from '../bindings';

export interface CustomError {
  kind: ErrorKind | 'walletconnect' | 'upload' | 'invalid';
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
    console.log('addError called with:', error);

    // Log current timestamp to help with timing
    console.log('addError timestamp:', new Date().toISOString());

    // Special handling for unauthorized errors to track their source
    if (error.kind === 'unauthorized') {
      console.log('🔍 UNAUTHORIZED ERROR DETECTED - ANALYZING SOURCE');
      console.log('Error details:', {
        kind: error.kind,
        reason: error.reason,
        timestamp: new Date().toISOString(),
      });

      // Try to get even more stack trace info
      const err = new Error('UNAUTHORIZED addError trace');
      if (Error.captureStackTrace) {
        Error.captureStackTrace(err, addError);
      }
      console.log('Enhanced stack trace:', err.stack);

      // Temporarily block this error from being added to see what happens
      console.log(
        '🚫 BLOCKING unauthorized error from being added to prevent dialog',
      );
      return; // Don't add the error
    }

    // Capture full stack trace with more details
    const stack = new Error('addError call trace').stack;
    console.log('Full stack trace:', stack);

    // Also capture console.trace for comparison
    console.trace('Console trace for addError call');

    setErrors((prevErrors) => [...prevErrors, error]);
  }, []);

  // Add global unhandled promise rejection handler
  useEffect(() => {
    const handleUnhandledRejection = (event: PromiseRejectionEvent) => {
      console.log('Unhandled promise rejection detected:', event.reason);
      console.log('Promise:', event.promise);

      // Check if this looks like our error
      if (event.reason && typeof event.reason === 'object') {
        const error = event.reason as CustomError;
        if (
          error.kind === 'unauthorized' ||
          error.kind === 'database_migration'
        ) {
          console.log('FOUND IT: Unhandled promise rejection is our error!');
          if (error.kind === 'unauthorized') {
            console.log('This is likely the source of our unauthorized error');
          }
        }
      }
    };

    window.addEventListener('unhandledrejection', handleUnhandledRejection);

    return () => {
      window.removeEventListener(
        'unhandledrejection',
        handleUnhandledRejection,
      );
    };
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
