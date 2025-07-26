import { create } from 'zustand';
import {
  Assets,
  commands,
  events,
  GetSyncStatusResponse,
  KeyInfo,
} from './bindings';
import { CustomError } from './contexts/ErrorContext';

export interface WalletState {
  sync: GetSyncStatusResponse;
}

export interface AssetInput {
  xch: string;
  cats: CatInput[];
  nfts: string[];
}

export interface CatInput {
  assetId: string;
  amount: string;
}

export interface OfferState {
  offered: Assets;
  requested: Assets;
  fee: string;
  expiration: OfferExpiration | null;
}

export interface OfferExpiration {
  days: string;
  hours: string;
  minutes: string;
}

export interface ReturnValue {
  status: 'success' | 'completed' | 'cancelled';
  data?: string;
}

export interface NavigationStore {
  returnValues: Record<string, ReturnValue>;
  setReturnValue: (pageId: string, value: ReturnValue) => void;
}

export const useWalletState = create<WalletState>(() => defaultState());
export const useOfferState = create<OfferState | null>(() => null);
export const useNavigationStore = create<NavigationStore>((set) => ({
  returnValues: {},
  setReturnValue: (pageId, value) =>
    set((state) => ({
      returnValues: { ...state.returnValues, [pageId]: value },
    })),
}));

export function clearState() {
  console.log('clearState: called');
  useWalletState.setState(defaultState());
  useOfferState.setState(null);
  console.log('clearState: completed');
}

export async function fetchState() {
  await Promise.all([updateSyncStatus()]);
}

let updateSyncStatusPromise: Promise<void> | null = null;

export function updateSyncStatus() {
  const callId = Math.random().toString(36).substr(2, 9);
  console.log(`updateSyncStatus: called [${callId}]`);
  // Prevent multiple concurrent calls
  if (updateSyncStatusPromise) {
    console.log(`updateSyncStatus: already running, returning [${callId}]`);
    return;
  }

  console.log(`updateSyncStatus: starting new call [${callId}]`);
  updateSyncStatusPromise = commands
    .getKey({})
    .then(async (keyData) => {
      console.log(`updateSyncStatus: getKey result [${callId}]:`, keyData);
      // Only fetch sync status if there's an authenticated wallet
      if (keyData.key) {
        console.log(`updateSyncStatus: calling getSyncStatus() [${callId}]`);
        try {
          const sync = await commands.getSyncStatus({});
          console.log(`updateSyncStatus: getSyncStatus completed [${callId}]`);
          useWalletState.setState({ sync });
        } catch (syncError) {
          console.log(`updateSyncStatus: getSyncStatus failed [${callId}]:`, syncError);
          throw syncError; // Re-throw to be caught by outer catch
        }
      } else {
        console.log(`updateSyncStatus: no key found, skipping getSyncStatus [${callId}]`);
      }
    })
    .catch((error) => {
      console.log(`updateSyncStatus: error caught [${callId}]:`, error);
      // Don't show unauthorized errors (happens when no wallet is logged in)
      if (error?.kind !== 'unauthorized') {
        console.error(`updateSyncStatus error [${callId}]:`, error);
      } else {
        console.log(`updateSyncStatus: ignoring unauthorized error [${callId}]`);
      }
    })
    .finally(() => {
      console.log(`updateSyncStatus: finally block, clearing promise [${callId}]`);
      updateSyncStatusPromise = null;
    });
}

events.syncEvent.listen((event) => {
  console.log('state.ts: syncEvent received:', event.payload.type);
  switch (event.payload.type) {
    case 'coin_state':
    case 'derivation':
    case 'puzzle_batch_synced':
    case 'nft_data':
      console.log('state.ts: calling updateSyncStatus()');
      updateSyncStatus();
      break;
  }
});

export async function loginAndUpdateState(
  fingerprint: number,
  onError?: (error: CustomError) => void,
): Promise<void> {
  try {
    await commands.login({ fingerprint });
    await fetchState();
  } catch (error) {
    if (onError) {
      onError(error as CustomError);
    } else {
      console.error(error);
    }
    throw error;
  }
}

// Create a separate function to handle wallet state updates
let setWalletState: ((wallet: KeyInfo | null) => void) | null = null;

export function initializeWalletState(
  setter: (wallet: KeyInfo | null) => void,
) {
  setWalletState = setter;
}

export async function logoutAndUpdateState(): Promise<void> {
  console.log('logoutAndUpdateState');
  try {
    clearState();
    if (setWalletState) {
      setWalletState(null);
    }
    await commands.logout({});
  } catch (error) {
    console.error('logoutAndUpdateState error:', error);
  }
}

export function defaultState(): WalletState {
  return {
    sync: {
      receive_address: 'Unknown',
      burn_address: 'Unknown',
      balance: '0',
      unit: {
        ticker: 'XCH',
        decimals: 12,
      },
      total_coins: 0,
      synced_coins: 0,
      unhardened_derivation_index: 0,
      hardened_derivation_index: 0,
      checked_files: 0,
      total_files: 0,
      database_size: 0,
    },
  };
}
