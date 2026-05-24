import { commands, NetworkKind } from '@/bindings';
import { useEffect, useState } from 'react';

export interface NetworkState {
  network: NetworkKind | null;
  isTestnet: boolean;
}

export function useNetwork(): NetworkState {
  const [network, setNetwork] = useState<NetworkKind | null>(null);

  useEffect(() => {
    commands
      .getNetwork({})
      .then((data) => setNetwork(data.kind))
      .catch((error) => {
        console.error('Failed to get network:', error);
        setNetwork('mainnet');
      });
  }, []);

  return { network, isTestnet: network === 'testnet' };
}
