import { commands } from '@/bindings';
import { CustomError } from '@/contexts/ErrorContext';
import { logoutAndUpdateState } from '@/state';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { useErrors } from './useErrors';

// Shared function to handle initialization errors
const handleInitializationError = async (
  error: unknown,
  addError: (error: CustomError) => void,
  setInitialized?: (value: boolean) => void,
) => {
  console.error('Error during initialization:', error);
  const customError = error as CustomError;

  // Check if this is a database migration, which is recoverable
  if (customError.kind === 'database_migration') {
    console.log('useInitialization: Handling database migration');
    try {
      await logoutAndUpdateState();
      console.log('useInitialization: logoutAndUpdateState completed');
      // Mark as initialized after successful logout
      if (setInitialized) {
        setInitialized(true);
        console.log('useInitialization: setInitialized(true) called');
      }
    } catch (logoutError) {
      console.error('Error during logout:', logoutError);
      // If logout fails, we should still try to continue
      if (setInitialized) {
        setInitialized(true);
        console.log('useInitialization: setInitialized(true) called after logout error');
      }
    }
  } else {
    // Only add non-migration errors to be displayed
    console.log('useInitialization: Adding non-migration error:', customError);
    addError(customError);
    console.error('Unrecoverable initialization error', error);
  }
};

export default function useInitialization() {
  const { addError } = useErrors();

  const [initialized, setInitialized] = useState(false);

  // Memoize addError to prevent unnecessary re-renders
  const memoizedAddError = useMemo(() => addError, [addError]);

  const onInitialize = useCallback(async () => {
    console.log('useInitialization: onInitialize called');
    try {
      console.log('useInitialization: calling commands.initialize()');
      await commands.initialize();
      console.log('useInitialization: commands.initialize() completed');
      setInitialized(true);
      console.log('useInitialization: setInitialized(true) after initialize');
      console.log('useInitialization: calling commands.switchWallet()');
      try {
        await commands.switchWallet();
        console.log('useInitialization: commands.switchWallet() completed');
      } catch (switchError) {
        console.log('useInitialization: commands.switchWallet() failed:', switchError);
        // switchWallet failure is not critical for initialization
      }
    } catch (error: unknown) {
      console.log('useInitialization: error caught, calling handleInitializationError');
      await handleInitializationError(error, memoizedAddError, setInitialized);
      console.log('useInitialization: handleInitializationError completed');
    }
  }, [memoizedAddError]);

  useEffect(() => {
    console.log('useInitialization: useEffect triggered, initialized =', initialized);
    if (!initialized) {
      console.log('useInitialization: calling onInitialize()');
      onInitialize();
    }
  }, [initialized, onInitialize]);

  return initialized;
}
