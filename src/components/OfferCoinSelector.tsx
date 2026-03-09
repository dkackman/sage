import { CoinRecord, commands, events } from '@/bindings';
import { fromMojos } from '@/lib/utils';
import { Assets } from '@/state';
import { t } from '@lingui/core/macro';
import { Trans } from '@lingui/react/macro';
import { RowSelectionState } from '@tanstack/react-table';
import BigNumber from 'bignumber.js';
import { ChevronDown, ChevronRight, Coins, XIcon } from 'lucide-react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { NumberFormat } from './NumberFormat';
import { SimplePagination } from './SimplePagination';
import { Button } from './ui/button';
import { Card, CardContent, CardHeader, CardTitle } from './ui/card';
import { Checkbox } from './ui/checkbox';

interface OfferCoinSelectorProps {
  offeredAssets: Assets;
  selectedCoinIds: string[];
  setSelectedCoinIds: (coinIds: string[]) => void;
}

interface TokenInfo {
  asset_id: string | null;
  name: string;
  ticker: string;
  precision: number;
}

const PAGE_SIZE = 5;

export function OfferCoinSelector({
  offeredAssets,
  selectedCoinIds,
  setSelectedCoinIds,
}: OfferCoinSelectorProps) {
  const [expanded, setExpanded] = useState(false);
  const [tokenInfos, setTokenInfos] = useState<TokenInfo[]>([]);

  // Deduplicate offered token asset_ids
  const offeredTokenAssetIds = useMemo(() => {
    const seen = new Set<string>();
    return offeredAssets.tokens
      .map((t) => t.asset_id)
      .filter((id) => {
        if (id === null || id === '') return false;
        // Use a string key for dedup; null won't reach here
        const key = id;
        if (seen.has(key)) return false;
        seen.add(key);
        return true;
      }) as string[];
  }, [offeredAssets.tokens]);

  // Include XCH if offered
  const hasXch = useMemo(
    () => offeredAssets.tokens.some((t) => t.asset_id === null),
    [offeredAssets.tokens],
  );

  const allAssetIds = useMemo(() => {
    const ids: (string | null)[] = [];
    if (hasXch) ids.push(null);
    ids.push(...offeredTokenAssetIds);
    return ids;
  }, [hasXch, offeredTokenAssetIds]);

  // Fetch token info for each offered asset
  useEffect(() => {
    if (allAssetIds.length === 0) {
      setTokenInfos([]);
      return;
    }

    Promise.all(
      allAssetIds.map((assetId) =>
        commands
          .getToken({ asset_id: assetId ?? null })
          .then((res) => ({
            asset_id: assetId,
            name: res.token?.name ?? (assetId === null ? 'Chia' : 'Unknown'),
            ticker:
              res.token?.ticker ?? (assetId === null ? 'XCH' : 'CAT'),
            precision: res.token?.precision ?? (assetId === null ? 12 : 3),
          }))
          .catch(() => ({
            asset_id: assetId,
            name: assetId === null ? 'Chia' : 'Unknown',
            ticker: assetId === null ? 'XCH' : 'CAT',
            precision: assetId === null ? 12 : 3,
          })),
      ),
    ).then(setTokenInfos);
  }, [allAssetIds]);

  // Clear selected coins for removed tokens
  useEffect(() => {
    if (allAssetIds.length === 0 && selectedCoinIds.length > 0) {
      setSelectedCoinIds([]);
    }
  }, [allAssetIds.length, selectedCoinIds.length, setSelectedCoinIds]);

  if (allAssetIds.length === 0) return null;

  return (
    <Card className='col-span-1 lg:col-span-2'>
      <CardHeader
        className='flex flex-row items-center justify-between space-y-0 pb-2 pr-2 space-x-2 cursor-pointer select-none'
        onClick={() => setExpanded(!expanded)}
      >
        <CardTitle className='text-md font-medium truncate flex items-center'>
          <Coins className='mr-2 h-4 w-4' />
          <Trans>Coin Selection</Trans>
          {selectedCoinIds.length > 0 && (
            <span className='ml-2 text-sm text-muted-foreground font-normal'>
              ({selectedCoinIds.length}{' '}
              {selectedCoinIds.length === 1 ? t`coin` : t`coins`})
            </span>
          )}
        </CardTitle>
        {expanded ? (
          <ChevronDown className='h-4 w-4 text-muted-foreground' />
        ) : (
          <ChevronRight className='h-4 w-4 text-muted-foreground' />
        )}
      </CardHeader>
      {expanded && (
        <CardContent>
          <div className='text-sm text-muted-foreground mb-4'>
            <Trans>
              Optionally select specific coins to use in this offer. When no
              coins are selected, the wallet will automatically choose which
              coins to spend.
            </Trans>
          </div>
          {tokenInfos.map((token) => (
            <TokenCoinSelector
              key={token.asset_id ?? 'xch'}
              token={token}
              selectedCoinIds={selectedCoinIds}
              setSelectedCoinIds={setSelectedCoinIds}
            />
          ))}
          {selectedCoinIds.length > 0 && (
            <div className='mt-2'>
              <Button
                variant='outline'
                size='sm'
                onClick={() => setSelectedCoinIds([])}
              >
                <XIcon className='h-3 w-3 mr-1' />
                <Trans>Clear All Selections</Trans>
              </Button>
            </div>
          )}
        </CardContent>
      )}
    </Card>
  );
}

interface TokenCoinSelectorProps {
  token: TokenInfo;
  selectedCoinIds: string[];
  setSelectedCoinIds: (coinIds: string[]) => void;
}

function TokenCoinSelector({
  token,
  selectedCoinIds,
  setSelectedCoinIds,
}: TokenCoinSelectorProps) {
  const [coins, setCoins] = useState<CoinRecord[]>([]);
  const [currentPage, setCurrentPage] = useState(0);
  const [totalCoins, setTotalCoins] = useState(0);
  const currentPageRef = useRef(currentPage);
  currentPageRef.current = currentPage;

  const fetchCoins = useCallback(
    (page: number = currentPageRef.current) => {
      commands
        .getCoins({
          asset_id: token.asset_id,
          offset: page * PAGE_SIZE,
          limit: PAGE_SIZE,
          sort_mode: 'amount',
          ascending: false,
          filter_mode: 'selectable',
        })
        .then((res) => {
          setCoins(res.coins);
          setTotalCoins(res.total);
        })
        .catch(console.error);
    },
    [token.asset_id],
  );

  useEffect(() => {
    fetchCoins();

    const unlisten = events.syncEvent.listen((event) => {
      const type = event.payload.type;
      if (type === 'coin_state' || type === 'puzzle_batch_synced') {
        fetchCoins();
      }
    });

    return () => {
      unlisten.then((u) => u());
    };
  }, [fetchCoins]);

  useEffect(() => {
    fetchCoins(currentPage);
  }, [currentPage, fetchCoins]);

  // Build a RowSelectionState for this token's coins
  const rowSelection: RowSelectionState = useMemo(() => {
    const selection: RowSelectionState = {};
    coins.forEach((coin) => {
      if (selectedCoinIds.includes(coin.coin_id)) {
        selection[coin.coin_id] = true;
      }
    });
    return selection;
  }, [coins, selectedCoinIds]);

  const toggleCoin = (coinId: string, selected: boolean) => {
    if (selected) {
      setSelectedCoinIds([...selectedCoinIds, coinId]);
    } else {
      setSelectedCoinIds(selectedCoinIds.filter((id) => id !== coinId));
    }
  };

  const toggleAllPage = (selected: boolean) => {
    if (selected) {
      const newIds = coins
        .map((c) => c.coin_id)
        .filter((id) => !selectedCoinIds.includes(id));
      setSelectedCoinIds([...selectedCoinIds, ...newIds]);
    } else {
      const pageIds = new Set(coins.map((c) => c.coin_id));
      setSelectedCoinIds(selectedCoinIds.filter((id) => !pageIds.has(id)));
    }
  };

  const allPageSelected =
    coins.length > 0 && coins.every((c) => selectedCoinIds.includes(c.coin_id));
  const somePageSelected =
    !allPageSelected &&
    coins.some((c) => selectedCoinIds.includes(c.coin_id));

  // Count and sum selected coins for this token
  const selectedForToken = useMemo(() => {
    // We need all coin records for selected IDs - some might not be on current page
    // For simplicity, count from selectedCoinIds that appear in any fetched page
    return coins.filter((c) => selectedCoinIds.includes(c.coin_id));
  }, [coins, selectedCoinIds]);

  const selectedCount = useMemo(() => {
    // Count all selected coins for this token across all pages
    // We know the coins on the current page; for a proper count we'd need all pages
    // For now, just show selected on current page + any others we know about
    return selectedCoinIds.filter(
      (id) =>
        coins.some((c) => c.coin_id === id) ||
        selectedForToken.some((c) => c.coin_id === id),
    ).length;
  }, [selectedCoinIds, coins, selectedForToken]);

  const selectedTotal = useMemo(() => {
    if (selectedForToken.length === 0) return null;
    const total = selectedForToken.reduce(
      (sum, coin) => sum.plus(coin.amount),
      new BigNumber(0),
    );
    return fromMojos(total, token.precision);
  }, [selectedForToken, token.precision]);

  const pageCount = Math.ceil(totalCoins / PAGE_SIZE);

  if (totalCoins === 0) return null;

  return (
    <div className='mb-4 last:mb-0'>
      <div className='text-sm font-medium mb-2 flex items-center justify-between'>
        <span>
          {token.name} ({token.ticker})
        </span>
        {selectedCount > 0 && selectedTotal && (
          <span className='text-muted-foreground font-normal'>
            {selectedCount} {selectedCount === 1 ? t`coin` : t`coins`} ={' '}
            <NumberFormat
              value={selectedTotal}
              minimumFractionDigits={0}
              maximumFractionDigits={token.precision}
            />{' '}
            {token.ticker}
          </span>
        )}
      </div>
      <div className='border rounded-lg overflow-hidden'>
        <table className='w-full text-sm'>
          <thead>
            <tr className='border-b bg-muted/50'>
              <th className='w-8 p-2'>
                <div className='flex justify-center'>
                  <Checkbox
                    checked={
                      allPageSelected ||
                      (somePageSelected && 'indeterminate')
                    }
                    onCheckedChange={(value) => toggleAllPage(!!value)}
                    aria-label={t`Select all coins on page`}
                  />
                </div>
              </th>
              <th className='p-2 text-left font-medium'>
                <Trans>Amount</Trans>
              </th>
              <th className='p-2 text-left font-medium'>
                <Trans>Coin ID</Trans>
              </th>
            </tr>
          </thead>
          <tbody>
            {coins.map((coin) => (
              <tr
                key={coin.coin_id}
                className={`border-b last:border-b-0 cursor-pointer hover:bg-muted/30 ${
                  rowSelection[coin.coin_id] ? 'bg-accent' : ''
                }`}
                onClick={() =>
                  toggleCoin(coin.coin_id, !rowSelection[coin.coin_id])
                }
              >
                <td className='w-8 p-2'>
                  <div className='flex justify-center'>
                    <Checkbox
                      checked={!!rowSelection[coin.coin_id]}
                      onCheckedChange={(value) =>
                        toggleCoin(coin.coin_id, !!value)
                      }
                      aria-label={t`Select coin ${coin.coin_id}`}
                      onClick={(e) => e.stopPropagation()}
                    />
                  </div>
                </td>
                <td className='p-2 font-mono'>
                  <NumberFormat
                    value={fromMojos(coin.amount, token.precision)}
                    minimumFractionDigits={0}
                    maximumFractionDigits={token.precision}
                  />
                </td>
                <td className='p-2 text-muted-foreground truncate max-w-[200px]'>
                  {coin.coin_id.slice(0, 8)}...{coin.coin_id.slice(-8)}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {pageCount > 1 && (
        <div className='pt-2'>
          <SimplePagination
            currentPage={currentPage}
            pageCount={pageCount}
            setCurrentPage={setCurrentPage}
            size='sm'
            align='between'
          />
        </div>
      )}
    </div>
  );
}
