# Passkey Unlock — Design

**Date:** 2026-08-03
**Branch:** `passkey-unlock` (off `password`)
**Status:** Approved design, ready for implementation planning

## Summary

Integrate the local (unpublished) `tauri-plugin-passkey` into sage and build the
first real feature on top of it: letting a user **unlock a password-protected key
with a passkey** (Face ID / Touch ID / security key) instead of typing the password.

The passkey is a _convenience door_ to the existing password gate — never a
replacement for it. The typed password always remains a working fallback, and the
key's at-rest encryption is unchanged. A secondary goal is served for free:
exercising the plugin in a real host app to shake out packaging/build/runtime
issues before it is published to crates.io / npm.

## Goals

- Wire `tauri-plugin-passkey` into sage as a local path/file dependency.
- Let a user enroll a passkey against a password-protected key, then unlock that
  key by passkey with the typed password as fallback.
- Leave `sage-keychain`'s crypto core and serialization untouched.
- First end-to-end target: **macOS**, reusing the existing relying party
  `webauthn.dkackman.com`.

## Non-goals

- Making the passkey a load-bearing factor (passkey-only keys, recovery flows).
- Envelope / multi-factor at-rest encryption (`password` and `passkey` as
  independent unlock factors of one key).
- A verifying WebAuthn relying-party server.
- Retiring or reworking the existing biometric gate.
- Protecting keys that have no password (passkey enrollment requires a real
  password).

## Chosen model

**"Passkey wraps the existing password."** The password stays the root secret.
Enrolling a passkey stores the key's password encrypted under a secret derived
from the passkey's PRF / hmac-secret extension. Unlocking by passkey recovers the
password, then decrypts through the existing path.

Rationale for this over the alternatives considered during brainstorming:

- _Envelope (either factor unlocks)_ — cleanest multi-factor model but requires a
  `KeyData::Secret` schema change and migration of every existing `keys.bin`.
  Rejected as too heavy for the first increment.
- _Passkey as alternative factor (swap the KEK source)_ — passkey **or** password,
  mutually exclusive per key; does not let the typed password remain a fallback.
  Rejected: we want the password to always work.
- _Passkey wraps the password (chosen)_ — near-zero change to the keychain crypto
  core; passkey is an alternate door to the same lock; password is always a
  fallback.

## Why no relying-party server is needed

A normal WebAuthn deployment needs a backend to mint challenges and verify
attestation/assertion signatures. This feature does not authenticate _to_ anything
— it only needs the PRF secret to be **stable** for a given `(credential, salt)`.
So sage generates the challenge and registration options locally, calls the
plugin, and extracts only `credential_id` and the PRF output. Security rests on:

1. the authenticator refusing to release PRF output without user verification
   (Face ID / Touch ID / device PIN), and
2. the wrapped password being useless ciphertext without that PRF secret.

No server, no network round-trip.

## Architecture (three layers)

### 1. Plugin wiring (mechanical)

Follows the pattern `tauri-plugin-sage` already uses, with the one wrinkle that
the plugin lives _outside_ the sage repo.

**Plugin modification is in-scope.** The plugin is unpublished and locally owned;
if the local-challenge / PRF-only flow (or any integration gap) needs plugin
changes, create a feature branch in `../../tauri-plugin-passkey` and modify it
there, iterating against sage via the path/file deps. Expectation from the
brainstorm is that the core flow needs no plugin change — its `register` /
`authenticate` API is a near drop-in for `@simplewebauthn/browser` and already
handles the PRF / hmac-secret extension on every platform — but the authorization
to change it exists if needed.

- `src-tauri/Cargo.toml`: `tauri-plugin-passkey = { path =
"../../tauri-plugin-passkey/tauri-plugin-passkey" }`. It is **not** a workspace
  member (path deps need not be), so it keeps its own edition/lints.
- `package.json`: `"tauri-plugin-passkey-api": "file:../tauri-plugin-passkey/tauri-plugin-passkey"`
  (its `dist-js/` must be built first — the plugin repo's `pnpm build` handles that).
- `src-tauri/src/lib.rs`: `.plugin(tauri_plugin_passkey::init())`.
- `src-tauri/capabilities/`: add `passkey:default` (platform capability as
  appropriate; macOS first).

### 2. Passkey-unlock service (frontend, new)

A `PasskeyContext` / `usePasskey` module beside `PasswordContext`, owning:

- `isEnrolled(fingerprint)` — reads wallet config.
- `enroll(fingerprint, password)` — register + PRF-eval + wrap.
- `unlock(fingerprint)` — assert + PRF-eval + unwrap → password.

`PasswordContext.requestPassword` gains a new **Case 0** at the top: if the key is
passkey-enrolled, try passkey first; on success resolve with the unwrapped
password; on cancel/error fall through to the existing password dialog. Everything
downstream still receives a password string, so `keychain.rs` and all
signing/login/delete endpoints are unchanged.

### 3. Enrollment record store (Rust, tiny)

Stored in `sage-config` per wallet — **not** in the secret keychain — because the
data is non-secret at rest (the wrapped password is ciphertext; credential id and
salt are public), and this keeps `sage-keychain` untouched. Tradeoff: the record
lives in a different file than the key; desync risk is low (same app dir) and a
reconcile step can be added later, mirroring the password-protection drift
reconciler.

## Data model

One optional field added to `Wallet` (`crates/sage-config/src/wallet.rs`, already
`serde` + `specta::Type`, so it flows to the TS bindings):

```rust
#[serde(skip_serializing_if = "Option::is_none")]
pub passkey: Option<PasskeyEnrollment>,

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PasskeyEnrollment {
    pub credential_id: String,     // base64url, from register()
    pub rp_id: String,             // relying-party id used at enroll
    pub prf_salt: String,          // base64, random per enrollment
    pub wrapped_password: String,  // base64: AES-256-GCM(nonce ‖ ciphertext) of the password
}
```

The PRF output is a 32-byte uniformly random secret, used **directly** as the
AES-256-GCM key — no HKDF and no new crypto deps beyond `aes-gcm`, already in the
tree. `expect-test` snapshots in `wallet.rs` will need updating for the new field.

## Rust endpoints (new, thin)

Keep the KDF/AEAD in Rust to match the existing keychain crypto style:

- `passkey_wrap_password(fingerprint, prf_secret, password)` — verify the password
  (existing `extract_secrets` probe), AES-256-GCM encrypt it under `prf_secret`,
  write the `PasskeyEnrollment` into wallet config.
- `passkey_unwrap_password(fingerprint, prf_secret)` — decrypt and return the
  password.

Enrollment read/removal go through existing wallet-config endpoints.

## Flows

### Enroll (Settings → "Unlock this key with a passkey"; offered only when the key is password-protected)

1. Prompt for the current password `P`; verify via `extract_secrets`.
2. Frontend generates a random `prf_salt`; calls plugin `register(rpOrigin,
options)` with the PRF extension → `credential_id`. Non-discoverable is fine
   (we store `credential_id`).
3. Frontend calls `authenticate(rpOrigin, { allowCredentials: [credential_id],
prf eval = salt })` → PRF secret `S`.
4. Frontend calls `passkey_wrap_password(fingerprint, S, P)` → enrollment stored.

### Unlock (Case 0 in `requestPassword`)

1. If passkey-enrolled: `authenticate(rpOrigin, { allowCredentials:
[credential_id], prf eval = salt })` → `S`; then
   `passkey_unwrap_password(fingerprint, S)` → `P`; resolve `P`.
2. On cancel/error: fall through to the existing password dialog. The typed
   password always works.

## Interaction with the existing biometric gate

The existing biometric gate (`BiometricContext` / `PasswordContext` Case 2) is
mobile-only and encrypts nothing — it is a _presence check_ in front of an
effectively-plaintext (empty-password) key. Passkey and biometric occupy different
axes: password and passkey-PRF both do real at-rest encryption; biometric only
proves presence.

Because passkey enrollment **requires** a password, every passkey-enrolled key is
already a has-password ("Case 1") key, so the Case 2 biometric standalone gate
never applies to them — no double-prompt, no conflict. Biometric stays exactly as
it is for password-less keys. Desktop is pure upside: biometric is unavailable on
desktop today, so passkey adds new capability there with no overlap.

## Correctness rule: stale wrapped password

The wrapped password goes stale if the key's password changes or is removed. Rule:

- **Changing or removing a key's password drops its passkey enrollment** (the user
  re-enrolls with one tap).
- **Deleting a key** cleans up its enrollment record.

## Platform / relying party

- rpId: reuse `webauthn.dkackman.com` (already the test-app's RP, developer mode).
  The AASA is served by a **Cloudflare Worker under our control**, so adding sage's
  Apple App ID to the association is ours to do — no external dependency.
- First target: **macOS**. Requires a signed dev bundle plus the
  `com.apple.developer.associated-domains` entitlement for the rpId, and sage's
  Apple App ID added to the AASA at `webauthn.dkackman.com` — mirroring the plugin
  test-app's `build-macos-dev.sh` setup.
- macOS dev-bundle signing scripts already exist in the plugin project and its
  sibling `secure-element` plugin; they can be adapted for sage. This signing step
  is the least-desirable part of the setup, so treat it as a fallback lever only if
  the entitlements route stalls.
- Fallback if macOS entitlement/signing becomes a yak-shave: the Linux/security-key
  CTAP2 path (no associated-domains), or proving the Rust core green first.
- PRF requires macOS 14+ (dev machine is 15+).

## Testing

- **Rust (no hardware):** wrap→unwrap round-trip; wrong PRF secret fails to
  decrypt; enroll rejects a wrong password; `expect-test` snapshot updates for
  `WalletConfig`.
- **Frontend:** Case 0 fallthrough — passkey cancel/unavailable silently falls to
  the password dialog.
- **Manual E2E on macOS:** enroll with Touch ID → relaunch → unlock by Touch ID →
  cancel → confirm typed-password fallback.

## Staged increments (become the implementation plan)

1. **Plugin wiring only** — Cargo `path` + `file:` deps, `init()`,
   `passkey:default` capability; build green on all platforms, no feature yet.
   _(reviewable checkpoint)_
2. **Rust core** — `PasskeyEnrollment` model, config plumbing,
   `passkey_wrap/unwrap_password` endpoints, unit tests, snapshot updates.
3. **macOS build enablement** — associated-domains entitlement, sage's App ID added
   to the `webauthn.dkackman.com` AASA, signed dev bundle.
4. **Frontend** — enroll UI in key settings, Case 0 unlock + fallback,
   drop-enrollment-on-password-change/remove wiring.
5. **Manual E2E verification on macOS.**

## Risks

- macOS entitlement/signing wiring (step 3) is the likeliest yak-shave.
- Enrollment/keychain desync (mitigated by drop-on-change rules; reconcile later).
- Plugin is unexercised outside its own test-app; integration bugs expected — that
  is partly the point of doing this before publishing.
