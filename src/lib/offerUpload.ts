import { OfferSummary } from '@/bindings';
import { OfferState } from '@/state';
import bs58 from 'bs58';

export async function getOfferHash(offer: string): Promise<string> {
  // Create SHA-256 hash of the UTF-8 encoded offer
  const encoder = new TextEncoder();
  const data = encoder.encode(offer);
  const hashBuffer = await crypto.subtle.digest('SHA-256', data);
  const hashArray = Array.from(new Uint8Array(hashBuffer));
  const hashBytes = new Uint8Array(hashArray);

  // Encode the hash in base58
  return bs58.encode(hashBytes);
}

export function isOneSideOffer(summary: OfferSummary | OfferState) {
  // Check if it's an OfferSummary
  if ('taker' in summary) {
    return !summary.taker.length;
  }

  // Handle OfferState
  return (
    summary.requested.tokens.filter(
      (t) => !!t.amount && (t.asset_id === null || !!t.asset_id),
    ).length === 0 && summary.requested.nfts.filter((n) => n).length === 0
  );
}

export function isMintGardenSupportedForSummary(summary: OfferSummary) {
  return (
    summary.maker.length === 1 &&
    summary.maker[0].asset.kind === 'nft' &&
    !isOneSideOffer(summary)
  );
}

export function isMintGardenSupported(state: OfferState, isSplitting = false) {
  if (isSplitting) {
    return (
      state.offered.tokens.filter(
        (t) => !!t.amount && (t.asset_id === null || !!t.asset_id),
      ).length === 0 &&
      state.offered.nfts.filter((n) => n).length > 0 &&
      !isOneSideOffer(state)
    );
  }
  return (
    state.offered.tokens.filter(
      (t) => !!t.amount && (t.asset_id === null || !!t.asset_id),
    ).length === 0 &&
    state.offered.nfts.filter((n) => n).length === 1 &&
    !isOneSideOffer(state)
  );
}

export function isDexieSupported(state: OfferState) {
  return !isOneSideOffer(state);
}

export function isDexieSupportedForSummary(summary: OfferSummary) {
  return !isOneSideOffer(summary);
}

export async function uploadToDexie(
  offer: string,
  testnet: boolean,
): Promise<string> {
  const response = await fetch(
    `https://${testnet ? 'api-testnet' : 'api'}.dexie.space/v1/offers`,
    {
      method: 'POST',
      body: JSON.stringify({ offer, drop_only: true }),
      headers: {
        'Content-Type': 'application/json',
      },
    },
  );

  const data = await response.json();
  if (!data?.success) {
    console.error(data);
    throw new Error(`Failed to upload offer to Dexie: ${data?.error_message}`);
  }

  return dexieLink(data.id, testnet);
}

export async function uploadToMintGarden(
  offer: string,
  testnet: boolean,
): Promise<string> {
  const response = await fetch(
    `https://${testnet ? 'api.testnet' : 'api'}.mintgarden.io/offer`,
    {
      method: 'POST',
      body: JSON.stringify({ offer }),
      headers: {
        'Content-Type': 'application/json',
      },
    },
  );

  const data = await response.json();
  if (!data?.offer?.id) {
    console.error(data);
    throw new Error(`Failed to upload offer to MintGarden: ${data?.detail}`);
  }

  return mintGardenLink(data.offer.id, testnet);
}

export function dexieLink(offerId: string, testnet: boolean) {
  return `https://${testnet ? 'testnet.' : ''}dexie.space/offers/${offerId}`;
}

export function mintGardenLink(offerHash: string, testnet: boolean) {
  return `https://${testnet ? 'testnet.' : ''}mintgarden.io/offers/${offerHash}`;
}

export async function offerIsOnDexie(
  offerId: string,
  isTestnet: boolean,
): Promise<boolean> {
  try {
    if (!offerId || offerId === '') return false;
    const response = await fetch(
      `https://${isTestnet ? 'api-testnet' : 'api'}.dexie.space/v1/offers/${offerId}`,
    );
    const data = await response.json();
    return data.success === true;
  } catch {
    return false;
  }
}

export async function offerIsOnMintGarden(
  offer: string,
  isTestnet: boolean,
): Promise<boolean> {
  try {
    if (!offer || offer === '') return false;
    const hash = await getOfferHash(offer);
    const response = await fetch(
      `https://api.${isTestnet ? 'testnet.' : ''}mintgarden.io/offers/${hash}`,
    );
    const data = await response.json();
    return data.id === hash;
  } catch {
    return false;
  }
}
