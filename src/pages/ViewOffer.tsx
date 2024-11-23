import {
  commands,
  Error,
  OfferAssets,
  OfferSummary,
  TakeOfferResponse,
} from '@/bindings';
import ConfirmationDialog from '@/components/ConfirmationDialog';
import Container from '@/components/Container';
import { CopyButton } from '@/components/CopyButton';
import ErrorDialog from '@/components/ErrorDialog';
import Header from '@/components/Header';
import { OfferCard } from '@/components/OfferCard';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Separator } from '@/components/ui/separator';
import { nftUri } from '@/lib/nftUri';
import { useWalletState } from '@/state';
import BigNumber from 'bignumber.js';
import { useEffect, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';

export function ViewOffer() {
  const { offer } = useParams();

  const navigate = useNavigate();

  const [summary, setSummary] = useState<OfferSummary | null>(null);
  const [response, setResponse] = useState<TakeOfferResponse | null>(null);
  const [error, setError] = useState<Error | null>(null);
  const [fee, setFee] = useState('');

  useEffect(() => {
    if (!offer) return;

    commands.viewOffer({ offer }).then((result) => {
      if (result.status === 'error') {
        setError(result.error);
      } else {
        setSummary(result.data.offer);
      }
    });
  }, [offer]);

  const importOffer = () => {
    commands.importOffer({ offer: offer! }).then((result) => {
      if (result.status === 'error') {
        setError(result.error);
      } else {
        navigate('/offers');
      }
    });
  };

  const take = () => {
    commands.importOffer({ offer: offer! }).then((result) => {
      if (result.status === 'error') {
        setError(result.error);
        return;
      }

      commands.takeOffer({ offer: offer!, fee: fee || '0' }).then((result) => {
        if (result.status === 'error') {
          setError(result.error);
        } else {
          setResponse(result.data);
        }
      });
    });
  };

  return (
    <>
      <Header title='View Offer' />

      <Container>
        {summary && (
          <OfferCard summary={summary}>
            <div className='flex flex-col space-y-1.5'>
              <Label htmlFor='fee'>Network Fee</Label>
              <Input
                id='fee'
                type='text'
                placeholder='0.00'
                className='pr-12'
                value={fee}
                onChange={(e) => setFee(e.target.value)}
                onKeyDown={(event) => {
                  if (event.key === 'Enter') {
                    event.preventDefault();
                    take();
                  }
                }}
              />

              <span className='text-xs text-muted-foreground'>
                {BigNumber(summary?.fee ?? '0').isGreaterThan(0)
                  ? `This does not include a fee of ${summary?.fee} which was already added by the maker.`
                  : ''}
              </span>
            </div>
          </OfferCard>
        )}

        <div className='mt-4 flex gap-2'>
          <Button variant='outline' onClick={importOffer}>
            Save Offer
          </Button>

          <Button onClick={take}>Take Offer</Button>
        </div>

        <ErrorDialog
          error={error}
          setError={(error) => {
            setError(error);
            if (error === null) navigate('/offers');
          }}
        />
      </Container>

      <ConfirmationDialog
        response={response}
        close={() => setResponse(null)}
        onConfirm={() => navigate('/offers')}
      />
    </>
  );
}

interface AssetsProps {
  assets: OfferAssets;
}

function Assets({ assets }: AssetsProps) {
  const walletState = useWalletState();

  const amount = BigNumber(assets.xch.amount);

  if (
    amount.isLessThanOrEqualTo(0) &&
    Object.keys(assets.cats).length === 0 &&
    Object.keys(assets.nfts).length === 0
  ) {
    return <></>;
  }

  return (
    <div className='flex flex-col gap-2 divide-neutral-200 dark:divide-neutral-800'>
      {amount.isGreaterThan(0) && (
        <div className='flex flex-col gap-1.5 rounded-md border p-2'>
          <div className='overflow-hidden flex items-center gap-2'>
            <Badge>
              <span className='truncate'>{walletState.sync.unit.ticker}</span>
            </Badge>

            <div className='text-sm font-medium'>
              {BigNumber(amount).plus(assets.xch.royalty).toString()}{' '}
              {walletState.sync.unit.ticker}
            </div>
          </div>

          {BigNumber(assets.xch.royalty).isGreaterThan(0) && (
            <>
              <Separator className='my-1' />

              <div className='text-sm text-muted-foreground truncate text-neutral-600 dark:text-neutral-300'>
                Amount includes {assets.xch.royalty}{' '}
                {walletState.sync.unit.ticker} royalty
              </div>
            </>
          )}
        </div>
      )}

      {Object.entries(assets.cats).map(([assetId, cat], i) => (
        <div key={i} className='flex flex-col gap-1.5 rounded-md border p-2'>
          <div className='overflow-hidden flex items-center gap-2'>
            <div className='truncate flex items-center gap-2'>
              <Badge className='max-w-[100px] bg-blue-600 text-white dark:bg-blue-600 dark:text-white'>
                <span className='truncate'>{cat.ticker ?? 'CAT'}</span>
              </Badge>
            </div>
            <div className='text-sm font-medium whitespace-nowrap'>
              {BigNumber(cat.amount).plus(cat.royalty).toString()} {cat.name}
            </div>
          </div>

          <Separator className='my-1' />

          <div className='flex gap-1.5 items-center'>
            {cat.icon_url && (
              <img src={cat.icon_url} className='w-6 h-6 rounded-full' />
            )}

            <div className='text-sm text-muted-foreground truncate font-mono'>
              {assetId.slice(0, 10) + '...' + assetId.slice(-10)}
            </div>

            <CopyButton value={assetId} className='w-4 h-4' />
          </div>

          {BigNumber(cat.royalty).isGreaterThan(0) && (
            <>
              <Separator className='my-1' />

              <div className='text-sm text-muted-foreground truncate text-neutral-600 dark:text-neutral-300'>
                Amount includes {cat.royalty} {cat.ticker ?? 'CAT'} royalty
              </div>
            </>
          )}
        </div>
      ))}

      {Object.entries(assets.nfts).map(([launcherId, nft], i) => (
        <div key={i} className='flex flex-col gap-1.5 rounded-md border p-2'>
          <div className='overflow-hidden flex items-center gap-2'>
            <div className='truncate flex items-center gap-2'>
              <Badge className='max-w-[100px] bg-green-600 text-white dark:bg-green-600 dark:text-white'>
                <span className='truncate'>NFT</span>
              </Badge>
            </div>

            <div className='text-sm font-medium'>{nft.name ?? 'Unnamed'}</div>
          </div>

          <Separator className='my-1' />

          <div className='flex gap-1.5 items-center'>
            <img
              src={nftUri(nft.image_mime_type, nft.image_data)}
              className='w-6 h-6 rounded-sm'
            />

            <div className='text-sm text-muted-foreground truncate font-mono'>
              {launcherId.slice(0, 10) + '...' + launcherId.slice(-10)}
            </div>

            <CopyButton value={launcherId} className='w-4 h-4' />
          </div>

          <Separator className='my-1' />

          <div className='flex gap-1.5 items-center text-sm text-muted-foreground truncate'>
            <span>
              <span className='text-neutral-600 dark:text-neutral-300'>
                {nft.royalty_percent}% royalty to{' '}
              </span>
              <span className='font-mono'>
                {nft.royalty_address.slice(0, 10) +
                  '...' +
                  nft.royalty_address.slice(-10)}
              </span>
            </span>
            <CopyButton value={nft.royalty_address} className='w-4 h-4' />
          </div>
        </div>
      ))}
    </div>
  );
}
