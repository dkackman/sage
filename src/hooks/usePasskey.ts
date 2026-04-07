import { useCallback } from 'react';

// TODO: replace with sage.rigidnetwork.com (or equivalent) before merging to main
const RP_ID = 'webauthn.dkackman.com';
const ORIGIN = `https://${RP_ID}`;

// --- Encoding helpers ---

function bytesToBase64url(bytes: Uint8Array): string {
  return btoa(String.fromCharCode(...bytes))
    .replace(/\+/g, '-')
    .replace(/\//g, '_')
    .replace(/=+$/, '');
}

function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('');
}

function hexToBytes(hex: string): Uint8Array {
  const bytes = new Uint8Array(hex.length / 2);
  for (let i = 0; i < hex.length; i += 2) {
    bytes[i / 2] = parseInt(hex.slice(i, i + 2), 16);
  }
  return bytes;
}

function base64urlToHex(b64url: string): string {
  const b64 = b64url
    .replace(/-/g, '+')
    .replace(/_/g, '/')
    .padEnd(b64url.length + ((4 - (b64url.length % 4)) % 4), '=');
  return bytesToHex(Uint8Array.from(atob(b64), (c) => c.charCodeAt(0)));
}

// --- Hook ---

export interface PasskeySetupResult {
  /** Hex-encoded WebAuthn credential ID */
  credentialId: string;
  /** Hex-encoded 32-byte PRF output */
  prfOutput: string;
  /** Hex-encoded 32-byte PRF salt */
  prfSalt: string;
}

export function usePasskey() {
  /**
   * Register a new passkey and immediately perform a PRF assertion to obtain
   * the encryption key material.
   *
   * Returns the credential ID, PRF output, and PRF salt — all hex-encoded —
   * ready to pass directly to `commands.setPasskey`.
   * Returns `undefined` if the authenticator does not support PRF, or if the
   * user cancels.
   */
  const setupPasskey = useCallback(
    async (
      fingerprint: number,
      username: string,
    ): Promise<PasskeySetupResult | undefined> => {
      try {
        const { register, authenticate } = await import(
          'tauri-plugin-webauthn-api'
        );

        const prfSaltBytes = crypto.getRandomValues(new Uint8Array(32));

        // Step 1: Register a new passkey, requesting PRF support
        const regResult = await register(ORIGIN, {
          rp: { name: 'Sage Wallet', id: RP_ID },
          user: {
            id: bytesToBase64url(
              new TextEncoder().encode(String(fingerprint)),
            ),
            name: username,
            displayName: username,
          },
          challenge: bytesToBase64url(
            crypto.getRandomValues(new Uint8Array(32)),
          ),
          pubKeyCredParams: [
            { alg: -7, type: 'public-key' },
            { alg: -257, type: 'public-key' },
          ],
          timeout: 60000,
          authenticatorSelection: {
            authenticatorAttachment: 'platform',
            residentKey: 'required',
            requireResidentKey: true,
            userVerification: 'required',
          },
          attestation: 'none',
          extensions: { hmacCreateSecret: true } as Record<string, unknown>,
        });

        // RegistrationExtensionsClientOutputs uses #[serde(rename_all = "camelCase")]
        // so hmac_secret → "hmacSecret" in the JSON response
        console.log('[usePasskey] register response:', JSON.stringify(regResult));
        const prfSupported = (
          regResult as { extensions?: { hmacSecret?: boolean } }
        ).extensions?.hmacSecret;
        if (!prfSupported) {
          return undefined;
        }

        // credentialId is base64url here; keep it that way for allowCredentials,
        // then convert to hex at the end for the sage API.
        const credentialIdB64url = regResult.id;

        // Step 2: Authenticate immediately to obtain the PRF output
        const authResult = await authenticate(ORIGIN, {
          rpId: RP_ID,
          challenge: bytesToBase64url(
            crypto.getRandomValues(new Uint8Array(32)),
          ),
          allowCredentials: [{ id: credentialIdB64url, type: 'public-key' }],
          timeout: 60000,
          userVerification: 'required',
          extensions: {
            hmacGetSecret: { output1: bytesToBase64url(prfSaltBytes) },
          } as Record<string, unknown>,
        });

        // AuthenticationExtensionsClientOutputs has NO rename_all, so the field
        // stays snake_case: "hmac_get_secret". HmacGetSecretOutput uses
        // rename_all = "camelCase" but "output1"/"output2" are already camelCase.
        const prfResult = (
          authResult as {
            extensions?: { hmac_get_secret?: { output1?: string } };
          }
        ).extensions?.hmac_get_secret?.output1;

        if (!prfResult) {
          return undefined;
        }

        return {
          credentialId: base64urlToHex(credentialIdB64url),
          prfOutput: base64urlToHex(prfResult),
          prfSalt: bytesToHex(prfSaltBytes),
        };
      } catch (e) {
        console.error('[usePasskey] setupPasskey error:', e);
        return undefined;
      }
    },
    [],
  );

  /**
   * Perform a PRF assertion for an existing passkey-protected wallet.
   *
   * Both `credentialIdHex` and `prfSaltHex` come directly from `KeyInfo`
   * (already hex-encoded). This function converts them to the base64url format
   * the WebAuthn plugin expects.
   *
   * Returns the hex-encoded PRF output, or `undefined` on failure/cancellation.
   */
  const authenticatePasskey = useCallback(
    async (
      credentialIdHex: string,
      prfSaltHex: string,
    ): Promise<string | undefined> => {
      try {
        const { authenticate } = await import('tauri-plugin-webauthn-api');

        const credentialIdB64url = bytesToBase64url(hexToBytes(credentialIdHex));
        const prfSaltB64url = bytesToBase64url(hexToBytes(prfSaltHex));

        const result = await authenticate(ORIGIN, {
          rpId: RP_ID,
          challenge: bytesToBase64url(
            crypto.getRandomValues(new Uint8Array(32)),
          ),
          allowCredentials: [{ id: credentialIdB64url, type: 'public-key' }],
          timeout: 60000,
          userVerification: 'required',
          extensions: {
            hmacGetSecret: { output1: prfSaltB64url },
          } as Record<string, unknown>,
        });

        const prfResult = (
          result as {
            extensions?: { hmac_get_secret?: { output1?: string } };
          }
        ).extensions?.hmac_get_secret?.output1;

        if (!prfResult) {
          return undefined;
        }

        return base64urlToHex(prfResult);
      } catch (e) {
        console.error('[usePasskey] authenticatePasskey error:', e);
        return undefined;
      }
    },
    [],
  );

  return { setupPasskey, authenticatePasskey };
}
