import {
  register,
  authenticate,
  isPasskeyError,
} from 'tauri-plugin-passkey-api';
import { commands, type KeyInfo, type PasskeyInfo } from '@/bindings';

export const RP_ID = 'webauthn.dkackman.com';
export const RP_ORIGIN = 'https://webauthn.dkackman.com';

/** Human-readable message for any error thrown by the passkey enroll/unlock flow. */
export function passkeyErrorMessage(e: unknown): string {
  if (isPasskeyError(e)) return `${e.kind}: ${e.message}`;
  if (e instanceof Error) return e.message;
  if (typeof e === 'string') return e;
  try {
    return JSON.stringify(e);
  } catch {
    return String(e);
  }
}

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

/** Pull the PRF output (base64url) from an assertion response. */
function prfSecretOutput(response: unknown): string | undefined {
  const r = response as {
    clientExtensionResults?: { prf?: { results?: { first?: string } } };
  };
  return r.clientExtensionResults?.prf?.results?.first;
}

/**
 * Whether the credential we just registered can actually do PRF. The plugin
 * reports `prf.enabled` after registration on every platform; a platform with
 * no PRF (e.g. Windows) succeeds registration but returns `enabled: false`
 * instead of silently dropping the extension. Treat an explicit `false` as
 * unsupported so we fail before asking for a secret we'll never get.
 */
function prfEnabledAfterRegistration(response: unknown): boolean | undefined {
  const r = response as {
    clientExtensionResults?: { prf?: { enabled?: boolean } };
  };
  return r.clientExtensionResults?.prf?.enabled;
}

/** Register a passkey for `fingerprint` and wrap `password` under its hmac-secret. */
export async function enrollPasskey(
  fingerprint: number,
  password: string,
): Promise<void> {
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
    authenticatorSelection: {
      // `preferred`, not `discouraged`: Android's Credential Manager only
      // creates discoverable credentials and the plugin now rejects
      // `discouraged` there outright. Unlock always uses `allowCredentials`
      // with the stored `credential_id`, so discoverability is immaterial to us.
      residentKey: 'preferred',
      userVerification: 'required',
    },
    attestation: 'none',
    timeout: 60000,
    // Enable the PRF extension on this credential.
    extensions: { prf: {} },
  } as PublicKeyCredentialCreationOptionsJSON);

  if (prfEnabledAfterRegistration(created) === false) {
    throw new Error(
      "This device can't unlock with a passkey — its authenticator doesn't support the PRF extension.",
    );
  }

  const credentialId = created.id;

  const assertion = await authenticate(RP_ORIGIN, {
    challenge: bytesToBase64Url(randomBytes(32)),
    rpId: RP_ID,
    allowCredentials: [{ type: 'public-key', id: credentialId }],
    userVerification: 'required',
    timeout: 60000,
    extensions: { prf: { eval: { first: prfSalt } } },
  } as PublicKeyCredentialRequestOptionsJSON);

  const secret = prfSecretOutput(assertion);
  if (!secret) throw new Error('Authenticator did not return a PRF secret');

  await commands.enrollPasskey({
    fingerprint,
    password,
    credential_id: credentialId,
    rp_id: RP_ID,
    prf_salt: prfSalt,
    prf_secret: bytesToStdBase64(base64UrlToBytes(secret)),
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

  const secret = prfSecretOutput(assertion);
  if (!secret) throw new Error('Authenticator did not return a PRF secret');

  const result = await commands.unwrapPasskeyPassword({
    fingerprint: info.fingerprint,
    prf_secret: bytesToStdBase64(base64UrlToBytes(secret)),
  });
  return result.password;
}
