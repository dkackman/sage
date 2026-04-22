import { useEffect, useMemo, useState } from 'react';
import { Amount, commands } from '@/bindings';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';

interface Props {
  appName: string;
  authorName?: string | null;
  authorAvatarSrc?: string | null;
  donationAddress: string;
  onSend: (amountMojos: Amount) => Promise<void> | void;
}

type DonationMode = 'usd' | 'xch';

function parsePositiveNumber(value: string): number | null {
  const normalized = value.trim();
  if (!normalized) {
    return null;
  }

  const parsed = Number(normalized);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    return null;
  }

  return parsed;
}

function xchToMojos(xch: number): Amount {
  return Math.floor(xch * 1_000_000_000_000);
}

export function AppDonationStrip({
  appName,
  authorName,
  authorAvatarSrc,
  donationAddress,
  onSend,
}: Props) {
  const [mode, setMode] = useState<DonationMode>('usd');
  const [usdInput, setUsdInput] = useState('10');
  const [xchInput, setXchInput] = useState('');
  const [priceUsd, setPriceUsd] = useState<number | null>(null);
  const [priceLoading, setPriceLoading] = useState(true);
  const [priceError, setPriceError] = useState<string | null>(null);
  const [sending, setSending] = useState(false);

  useEffect(() => {
    let cancelled = false;

    async function loadPrice() {
      try {
        setPriceLoading(true);
        setPriceError(null);

        const response = await commands.getXchUsdPrice({});

        if (!cancelled) {
          setPriceUsd(response.usd);
        }
      } catch (err) {
        if (!cancelled) {
          setPriceError(err instanceof Error ? err.message : String(err));
          setPriceUsd(null);
          setMode('xch');
        }
      } finally {
        if (!cancelled) {
          setPriceLoading(false);
        }
      }
    }

    void loadPrice();

    return () => {
      cancelled = true;
    };
  }, []);

  const derived = useMemo(() => {
    if (mode === 'usd') {
      const usd = parsePositiveNumber(usdInput);
      if (!usd || !priceUsd) {
        return {
          usd: usd,
          xch: null as number | null,
          mojos: null as string | null,
        };
      }

      const xch = usd / priceUsd;
      return {
        usd,
        xch,
        mojos: xchToMojos(xch),
      };
    }

    const xch = parsePositiveNumber(xchInput);
    if (!xch) {
      return {
        usd: null as number | null,
        xch,
        mojos: null as string | null,
      };
    }

    return {
      usd: priceUsd ? xch * priceUsd : null,
      xch,
      mojos: xchToMojos(xch),
    };
  }, [mode, usdInput, xchInput, priceUsd]);

  const canSend =
    !sending &&
    derived.xch !== null &&
    derived.mojos !== null &&
    Number(derived.mojos) > 0;

  async function handleSend() {
    if (!canSend || !derived.mojos) {
      return;
    }

    try {
      setSending(true);
      await onSend(derived.mojos);
    } finally {
      setSending(false);
    }
  }

  return (
    <div className='shrink-0 border-b bg-amber-500/5 px-4 py-3'>
      <div className='flex flex-wrap items-center gap-4'>
        <div className='flex min-w-0 items-center gap-3'>
          {authorAvatarSrc ? (
            <img
              src={authorAvatarSrc}
              alt=''
              className='h-10 w-10 rounded-full border object-cover'
            />
          ) : (
            <div className='flex h-10 w-10 items-center justify-center rounded-full border bg-background text-sm font-semibold'>
              {(authorName ?? appName).slice(0, 1).toUpperCase()}
            </div>
          )}

          <div className='min-w-0'>
            <div className='truncate text-sm font-semibold'>
              Support {authorName ?? appName}
            </div>
            <div className='truncate text-xs text-muted-foreground'>
              {donationAddress}
            </div>
            <div className='text-xs text-muted-foreground'>
              {priceLoading
                ? 'Loading XCH price…'
                : priceUsd !== null
                  ? `1 XCH ≈ $${priceUsd.toFixed(2)}`
                  : 'XCH price unavailable'}
            </div>
          </div>
        </div>

        <div className='ml-auto flex flex-wrap items-center gap-2'>
          <div className='flex items-center gap-1 rounded-lg border bg-background p-1'>
            <Button
              type='button'
              size='sm'
              variant={mode === 'usd' ? 'default' : 'ghost'}
              disabled={priceUsd === null}
              onClick={() => setMode('usd')}
            >
              $
            </Button>
            <Button
              type='button'
              size='sm'
              variant={mode === 'xch' ? 'default' : 'ghost'}
              onClick={() => setMode('xch')}
            >
              XCH
            </Button>
          </div>

          <Input
            className='w-28'
            value={mode === 'usd' ? usdInput : xchInput}
            onChange={(event) => {
              if (mode === 'usd') {
                setUsdInput(event.target.value);
              } else {
                setXchInput(event.target.value);
              }
            }}
            placeholder={mode === 'usd' ? '10.00' : '0.100000'}
          />

          <div className='min-w-[120px] text-right text-xs text-muted-foreground'>
            {mode === 'usd'
              ? derived.xch !== null
                ? `≈ ${derived.xch.toFixed(6)} XCH`
                : '—'
              : derived.usd !== null
                ? `≈ $${derived.usd.toFixed(2)}`
                : '—'}
          </div>

          <Button onClick={() => void handleSend()} disabled={!canSend}>
            {sending ? 'Sending…' : 'Send support'}
          </Button>
        </div>
      </div>

      {priceError ? (
        <div className='mt-2 text-xs text-muted-foreground'>
          Price lookup failed. XCH mode still works.
        </div>
      ) : null}
    </div>
  );
}
