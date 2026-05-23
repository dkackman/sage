import { commands, NetworkKind } from '@/bindings';
import { useEffect, useState } from 'react';

export function useNetwork(): NetworkKind | null {
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

  return network;
}
