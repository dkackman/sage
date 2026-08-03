import { register, authenticate } from 'tauri-plugin-passkey-api';
import { commands, type KeyInfo, type PasskeyInfo } from '@/bindings';

export const RP_ID = 'webauthn.dkackman.com';
export const RP_ORIGIN = 'https://webauthn.dkackman.com';

function randomBytes(len: number): Uint8Array {
  const b = new Uint8Array(len);
  crypto.getRandomValues(b);
  return b;
}

function bytesToBase64Url(bytes: Uint8Array): string {
  let bin = '';
  for (const byte of bytes) bin += String.fromCharCode(byte);
  return btoa(bin).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

function base64UrlToBytes(s: string): Uint8Array {
  const pad = s.length % 4 === 0 ? '' : '='.repeat(4 - (s.length % 4));
  const bin = atob(s.replace(/-/g, '+').replace(/_/g, '/') + pad);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

function bytesToStdBase64(bytes: Uint8Array): string {
  let bin = '';
  for (const byte of bytes) bin += String.fromCharCode(byte);
  return btoa(bin);
}

// The plugin returns WebAuthn JSON; PRF results arrive as base64url strings.
function prfFirst(response: unknown): string | undefined {
  const ext = (response as { clientExtensionResults?: { prf?: { results?: { first?: string } } } })
    .clientExtensionResults;
  return ext?.prf?.results?.first;
}

/** Register a passkey for `fingerprint` and wrap `password` under its PRF secret. */
export async function enrollPasskey(fingerprint: number, password: string): Promise<void> {
  const prfSalt = bytesToBase64Url(randomBytes(32));

  const created = await register(RP_ORIGIN, {
    rp: { id: RP_ID, name: 'Sage Wallet' },
    user: {
      id: bytesToBase64Url(randomBytes(16)),
      name: `sage-key-${fingerprint}`,
      displayName: `Sage key ${fingerprint}`,
    },
    challenge: bytesToBase64Url(randomBytes(32)),
    pubKeyCredParams: [
      { type: 'public-key', alg: -7 },
      { type: 'public-key', alg: -257 },
    ],
    authenticatorSelection: { residentKey: 'discouraged', userVerification: 'required' },
    attestation: 'none',
    timeout: 60000,
    extensions: { prf: { eval: { first: prfSalt } } },
  } as PublicKeyCredentialCreationOptionsJSON);

  const credentialId = created.id;

  const assertion = await authenticate(RP_ORIGIN, {
    challenge: bytesToBase64Url(randomBytes(32)),
    rpId: RP_ID,
    allowCredentials: [{ type: 'public-key', id: credentialId }],
    userVerification: 'required',
    timeout: 60000,
    extensions: { prf: { eval: { first: prfSalt } } },
  } as PublicKeyCredentialRequestOptionsJSON);

  const prf = prfFirst(assertion);
  if (!prf) throw new Error('Authenticator did not return a PRF secret');

  await commands.enrollPasskey({
    fingerprint,
    password,
    credential_id: credentialId,
    rp_id: RP_ID,
    prf_salt: prfSalt,
    prf_secret: bytesToStdBase64(base64UrlToBytes(prf)),
  });
}

/** Unlock a passkey-enrolled key; returns the recovered password. */
export async function unlockWithPasskey(info: KeyInfo): Promise<string> {
  const enrollment: PasskeyInfo | null = info.passkey;
  if (!enrollment) throw new Error('Key has no passkey enrollment');

  const assertion = await authenticate(RP_ORIGIN, {
    challenge: bytesToBase64Url(randomBytes(32)),
    rpId: enrollment.rp_id,
    allowCredentials: [{ type: 'public-key', id: enrollment.credential_id }],
    userVerification: 'required',
    timeout: 60000,
    extensions: { prf: { eval: { first: enrollment.prf_salt } } },
  } as PublicKeyCredentialRequestOptionsJSON);

  const prf = prfFirst(assertion);
  if (!prf) throw new Error('Authenticator did not return a PRF secret');

  const result = await commands.unwrapPasskeyPassword({
    fingerprint: info.fingerprint,
    prf_secret: bytesToStdBase64(base64UrlToBytes(prf)),
  });
  return result.password;
}
