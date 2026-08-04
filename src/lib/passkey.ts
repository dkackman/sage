import { register, authenticate, isPasskeyError } from 'tauri-plugin-passkey-api';
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

// This plugin does NOT use the browser `prf` extension shape. It deserializes
// options into webauthn-rs-proto types, whose hmac-secret extension is:
//   register:     extensions.hmacCreateSecret = true          (camelCase input)
//   authenticate: extensions.hmacGetSecret    = { output1 }   (camelCase input, salt in output1)
// and returns the secret in the response as (snake_case output):
//   response.extensions.hmac_get_secret.output1  (base64url)
// See webauthn-rs-proto 0.5 extensions.rs and the plugin's macos.rs.
const HMAC_CREATE_SECRET = { hmacCreateSecret: true };

function hmacGetSecretInput(saltBase64Url: string) {
  return { hmacGetSecret: { output1: saltBase64Url } };
}

/** Pull the hmac-secret (PRF) output (base64url) from an assertion response. */
function hmacSecretOutput(response: unknown): string | undefined {
  const r = response as {
    extensions?: Record<string, unknown>;
    clientExtensionResults?: Record<string, unknown>;
  };
  const container = r?.extensions ?? r?.clientExtensionResults;
  const hmac = (container?.hmac_get_secret ?? container?.hmacGetSecret) as
    | { output1?: string; first?: string }
    | undefined;
  return hmac?.output1 ?? hmac?.first;
}

/** Register a passkey for `fingerprint` and wrap `password` under its hmac-secret. */
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
    // requireResidentKey is deprecated in the WebAuthn spec (superseded by
    // residentKey), but the plugin's webauthn-rs-proto wire contract marks it
    // required, so it must be sent explicitly. false matches 'discouraged'.
    authenticatorSelection: {
      residentKey: 'discouraged',
      requireResidentKey: false,
      userVerification: 'required',
    },
    attestation: 'none',
    timeout: 60000,
    extensions: HMAC_CREATE_SECRET,
  } as unknown as PublicKeyCredentialCreationOptionsJSON);

  const credentialId = created.id;

  const assertion = await authenticate(RP_ORIGIN, {
    challenge: bytesToBase64Url(randomBytes(32)),
    rpId: RP_ID,
    allowCredentials: [{ type: 'public-key', id: credentialId }],
    userVerification: 'required',
    timeout: 60000,
    extensions: hmacGetSecretInput(prfSalt),
  } as unknown as PublicKeyCredentialRequestOptionsJSON);

  const secret = hmacSecretOutput(assertion);
  if (!secret) throw new Error('Authenticator did not return an hmac-secret output');

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
    extensions: hmacGetSecretInput(enrollment.prf_salt),
  } as unknown as PublicKeyCredentialRequestOptionsJSON);

  const secret = hmacSecretOutput(assertion);
  if (!secret) throw new Error('Authenticator did not return an hmac-secret output');

  const result = await commands.unwrapPasskeyPassword({
    fingerprint: info.fingerprint,
    prf_secret: bytesToStdBase64(base64UrlToBytes(secret)),
  });
  return result.password;
}
