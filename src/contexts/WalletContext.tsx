import { KeyInfo, commands } from '@/bindings';
import { CustomError } from '@/contexts/ErrorContext';
import { useErrors } from '@/hooks/useErrors';
import {
  fetchState,
  initializeWalletState,
  logoutAndUpdateState,
} from '@/state';
import { createContext, useContext, useEffect, useState } from 'react';

interface WalletContextType {
  wallet: KeyInfo | null;
  setWallet: (wallet: KeyInfo | null) => void;
}

export const WalletContext = createContext<WalletContextType | undefined>(
  undefined,
);

export function WalletProvider({ children }: { children: React.ReactNode }) {
  const [wallet, setWallet] = useState<KeyInfo | null>(null);
  const [hasInitialized, setHasInitialized] = useState(false);
  const { addError } = useErrors();

  useEffect(() => {
    // Only initialize once
    if (hasInitialized) return;

    const init = async () => {
      console.log('WalletContext: Starting initialization');
      try {
        initializeWalletState(setWallet);
        const data = await commands.getKey({});
        setWallet(data.key);
        if (data.key) {
          console.log('WalletContext: Found key, calling fetchState()');
          try {
            await fetchState();
            console.log('WalletContext: fetchState() completed');
          } catch (fetchError) {
            console.log('WalletContext: fetchState() failed:', fetchError);
            const fetchCustomError = fetchError as CustomError;
            if (
              fetchCustomError.kind === 'unauthorized' ||
              fetchCustomError.kind === 'database_migration'
            ) {
              console.log(
                'WalletContext: fetchState failed with auth/migration error, clearing wallet',
              );
              setWallet(null); // Clear the wallet since it's in a broken state
            } else {
              throw fetchError; // Re-throw non-auth errors
            }
          }
        }
        setHasInitialized(true);
        console.log('WalletContext: Initialization completed');
      } catch (error) {
        const customError = error as CustomError;

        // Handle database migration errors without showing them to user
        if (customError.kind === 'database_migration') {
          console.log('Database migration needed during wallet initialization');
          try {
            await logoutAndUpdateState();
            console.log('WalletContext: logout completed successfully');
          } catch (logoutError) {
            console.error('Error during logout:', logoutError);
          }
        } else {
          // Only add non-migration errors to be displayed
          console.log(
            'WalletContext: adding non-migration error:',
            customError,
          );
          addError(customError);
        }
        setHasInitialized(true);
      }
    };

    init();
  }, [hasInitialized, addError]);

  return (
    <WalletContext.Provider value={{ wallet, setWallet }}>
      {children}
    </WalletContext.Provider>
  );
}

export function useWallet() {
  const context = useContext(WalletContext);
  if (context === undefined) {
    throw new Error('useWallet must be used within a WalletProvider');
  }
  return context;
}
