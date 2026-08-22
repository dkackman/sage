# Password Protection for Sage Wallet

**Issue:** [xch-dev/sage#206](https://github.com/xch-dev/sage/issues/206)
**Date:** 2026-03-15
**Status:** Implemented; the frontend half superseded by
[2026-08-21-password-gate-design.md](2026-08-21-password-gate-design.md)

> **What is still current:** the keychain and encryption model, the sentinel convention, the set of
> protected operations, and password management in Settings.
>
> **What has moved:** who decides that authentication is needed, and who collects it. This document
> describes a frontend that calls `requestPassword(hasPassword)` at ~16 call sites and threads the
> result into each command. That is no longer how it works. Rust now decides and prompts, and no
> caller supplies a password. The sections below marked _(superseded)_ are kept for the history of
> why the backend looks the way it does; read the password-gate design for current behaviour.

## Overview

Add opt-in password protection to Sage wallet, requiring authentication for three categories of sensitive operations: displaying secrets, signing transactions/offers, and generating hardened keys. Biometric unlock (Touch ID, Face ID) is available as a standalone gate for wallets without passwords. Biometric and password are mutually exclusive — password takes precedence.

The prompt itself is now driven from Rust rather than from the call site; the decision tree below survives intact, but it runs inside `PasswordContext` as a _responder_ to a `PasswordRequest` event.

## Design Decisions

- **Per-operation authentication** — every protected operation prompts for the password. No session caching.
- **Opt-in** — existing wallets continue working without a password. Users can enable protection via "Set Password."
- **Per-key passwords** — each key in the keychain has its own password (or no password). This follows from the existing data model where each `KeyData::Secret` has its own `Encrypted` struct with its own salt. Which key's password is being asked for is resolved in Rust: the gate uses the active wallet, except for endpoints carrying a `fingerprint`, which name their own wallet.
- **Biometric is mutually exclusive with password** — biometric is a standalone gate for no-password wallets. If a wallet has a password, the password dialog is always shown regardless of biometric settings. The two never interact.

## Architecture

### Password Is Never Stored (Backend)

The password is a transient input, not persisted state. The existing encryption infrastructure in `sage-keychain` handles everything:

1. At key import (or password set): Argon2 derives a 256-bit AES key from `password + random 32-byte salt`
2. The wallet secret (mnemonic entropy or raw secret key) is encrypted with AES-256-GCM
3. `keys.bin` stores only `{ciphertext, nonce, salt}` — no password, no derived key
4. On each protected operation: user provides password, Argon2 re-derives the key, AES-GCM decrypts. Wrong password fails AES-GCM authentication.
5. Argon2 default parameters provide computational cost that mitigates brute-force attempts against the encrypted data at rest.

### Password Sentinel Convention

The empty byte string `b""` is the "no password" sentinel. This is what existing keys are encrypted with today. The convention is:

- `Option<String>` in request structs: `None` and `Some("")` both map to `b""` at the backend via `req.password.unwrap_or_default().into_bytes()`
- `ChangePassword` uses `String` (not `Option`): an empty `old_password` means the key currently has no password; an empty `new_password` removes password protection

### Has-Password Indicator

`KeyInfo` carries `has_password: bool`, returned by `get_key()` / `get_keys()`.

**As implemented**, the flag is read from `Wallet::password_protected` in the wallet config
(`crates/sage/src/endpoints/keys.rs:357` and `:419`) — a cheap field lookup, not a trial decrypt.
`keys.bin` is untouched; `KeyData::Secret` has no such field. An early draft proposed storing it in
`KeyData::Secret`, which would have changed the `keys.bin` serialization format and required a
versioned deserialization fallback. That draft was not built.

Because a config file and a `keys.bin` can drift apart, two things reconcile the flag against
reality, both by trial-decrypting with `b""` via `Keychain::is_password_protected`:

- `Sage::switch_wallet` self-heals on login.
- The `reconcile_key_protection` endpoint self-heals on demand. The frontend calls it from
  `ErrorContext` when a decrypt fails on a wallet whose flag says "no password" — that mismatch is
  the drift signal — so the next attempt prompts correctly.

### Biometric Gate (Mobile)

Biometric is a frontend-only concern, mutually exclusive with password. It serves as a standalone gate for wallets that do not have a password set.

**Global setting:** Biometric unlock is a single global toggle (the existing `useLocalStorage('biometric', false)` flag). It is only visible on mobile when biometric hardware is available and enrolled.

**Mutual exclusivity rule:** If a wallet has a password, the password dialog is always shown — biometric is irrelevant. Biometric only applies when `hasPassword` is false.

**No keychain storage:** Passwords are never stored on device. Each password-protected operation prompts via the password dialog. There is no keychain bridge between biometric and password.

**Biometric caching:** Standalone biometric prompts use a 5-minute cache (`performance.now()` monotonic clock) to avoid prompting repeatedly for rapid successive operations.

## Protected Operations

There are 8 `extract_secrets` call sites, plus 2 encrypt-at-creation sites that still use the `b""`
sentinel. Because `sign()` is reached through `transact()` and `transact_with()`, the password flows
through a much larger API surface than that count suggests.

### 1. Display mnemonic/secret key (1 site)

| Call site                               | Function           |
| --------------------------------------- | ------------------ |
| `crates/sage/src/endpoints/keys.rs:367` | `get_secret_key()` |

### 2. Sign transactions and offers

The central signing path is:

```text
endpoint method → transact() / transact_with() → sign() → extract_secrets()
```

**Direct `extract_secrets` call sites** (5 sites):

| Call site                                         | Function                                                        |
| ------------------------------------------------- | --------------------------------------------------------------- |
| `crates/sage/src/utils/spends.rs:17`              | `sign()` — called by `transact_with()` and `sign_coin_spends()` |
| `crates/sage/src/endpoints/offers.rs:172`         | `make_offer()` — calls `extract_secrets` directly               |
| `crates/sage/src/endpoints/offers.rs:212`         | `take_offer()` — calls `extract_secrets` directly               |
| `crates/sage/src/endpoints/wallet_connect.rs:186` | `sign_message_with_public_key()`                                |
| `crates/sage/src/endpoints/wallet_connect.rs:227` | `sign_message_by_address()`                                     |
| `crates/sage/src/endpoints/keys.rs:279`           | `delete_key()` — verifies before an irreversible delete         |

**Transaction endpoints that flow through `transact()` → `sign()`** (21 endpoints):

`send_xch`, `bulk_send_xch`, `combine`, `auto_combine_xch`, `split`, `auto_combine_cat`, `issue_cat`, `send_cat`, `bulk_send_cat`, `multi_send`, `create_did`, `bulk_mint_nfts`, `transfer_nfts`, `add_nft_uri`, `assign_nfts_to_did`, `transfer_dids`, `normalize_dids`, `mint_option`, `transfer_options`, `exercise_options`, `finalize_clawback`

Plus `cancel_offer`, `cancel_offers`, and `create_transaction` (action system) which also flow through `transact()` / `transact_with()`.

### 3. Delete wallet key

Password-protected wallets require password verification before deletion. This is enforced in Rust:
`delete_key()` takes a `password` and calls `extract_secrets` itself when
`keychain.is_password_protected(req.fingerprint)`
(`crates/sage/src/endpoints/keys.rs:277`). Deletion is irreversible, so it does not trust a caller to
have checked.

An earlier draft verified on the frontend by calling `get_secret_key()` first and blocking the delete
if decryption failed. `WalletCard.deleteSelf()` now simply calls `deleteKey`.

| Call site        | Function                                            |
| ---------------- | --------------------------------------------------- |
| `WalletCard.tsx` | `deleteSelf()` — calls `deleteKey` and nothing else |

### 4. Generate hardened keys (1 site)

| Call site                                  | Function                      |
| ------------------------------------------ | ----------------------------- |
| `crates/sage/src/endpoints/actions.rs:204` | `increase_derivation_index()` |

### 5. Key import — encrypt at creation (2 sites)

| Call site                               | Function                         |
| --------------------------------------- | -------------------------------- |
| `crates/sage/src/endpoints/keys.rs:143` | `import_key()` — secret key path |
| `crates/sage/src/endpoints/keys.rs:180` | `import_key()` — mnemonic path   |

**Import takes no password.** Both sites pass the `b""` sentinel, and `ImportKey` has no `password`
field at all — an early draft added one, but the shipped design sets a password afterwards through
Settings instead. This is why `import_key` is absent from the gating manifest.

Note: `import_key()` also generates hardened derivations using the in-memory master key during import. This does NOT need the password since the key is already decrypted at that point.

## Changes

### `sage-keychain` crate

**`keychain.rs`** — Add one new method:

```rust
pub fn change_password(
    &mut self,
    fingerprint: u32,
    old_password: &[u8],
    new_password: &[u8],
) -> Result<(), KeychainError>
```

Decrypts with old password, re-encrypts with new password, replaces the `KeyData::Secret` entry.

**`key_data.rs`** — Unchanged. An earlier draft of this design added `password_protected: bool` to `KeyData::Secret`, but that changes the `keys.bin` serialization format and would require a versioned deserialization fallback for existing files. Instead the flag lives in the wallet config (see below), leaving `keys.bin` format-compatible.

### `sage-config` crate

**`wallet.rs`** — Add `password_protected: bool` to `Wallet` (defaults to `false`, so existing `config.toml` files deserialize unchanged).

Because the config file and `keys.bin` can drift apart (e.g. a restored `keys.bin`), `Sage::switch_wallet` self-heals the flag via `Keychain::is_password_protected`, which trial-decrypts the entry with an empty password.

### `sage-api` crate (request structs)

Add `password: Option<String>` to **all request structs that trigger signing, secret access, or key import**:

**Direct secret access:**

- `GetSecretKey`
- `DeleteKey` — deletion is irreversible, so `delete_key` verifies the password itself rather than trusting the frontend to have checked

**Signing via `transact()` path — all transaction request structs:**

- `SendXch`, `BulkSendXch`, `Combine`, `AutoCombineXch`, `Split`, `AutoCombineCat`, `IssueCat`, `SendCat`, `BulkSendCat`, `MultiSend`, `CreateDid`, `BulkMintNfts`, `TransferNfts`, `AddNftUri`, `AssignNftsToDid`, `TransferDids`, `NormalizeDids`, `MintOption`, `TransferOptions`, `ExerciseOptions`, `FinalizeClawback`

**Signing via direct `extract_secrets` or `sign()`:**

- `SignCoinSpends`, `MakeOffer`, `TakeOffer`, `CancelOffer`, `CancelOffers`, `CreateTransaction`

**Hardened derivation:**

- `IncreaseDerivationIndex`

**WalletConnect signing:**

- `SignMessageWithPublicKey`, `SignMessageByAddress`

**New request/response pairs:**

- `ChangePassword { fingerprint: u32, old_password: String, new_password: String }`
- `ChangePasswordResponse {}`
- `ReconcileKeyProtection { fingerprint: u32 }`
- `ReconcileKeyProtectionResponse { has_password: bool }`

**`KeyInfo`** — add `has_password: bool` field.

Every request type carrying `password: Option<String>` is now also an entry in
`crates/sage-api/password-gating.json`, and a drift test fails the build if the two sets diverge. The
field stays because `sage-rpc` clients supply it over mTLS; the Tauri host layer overwrites it with a
password it resolved itself.

### `sage` crate (endpoints)

**`spends.rs`**: `sign()` takes `password: &[u8]` parameter, passes to `extract_secrets`.

**`transactions.rs`**: `transact()` and `transact_with()` take `password: &[u8]` parameter, pass to `sign()`. Every transaction endpoint extracts password from its request struct via `req.password.unwrap_or_default().into_bytes()` and passes to `transact()`.

**`keys.rs`**: `get_secret_key()` and `delete_key()` pass password to `extract_secrets()`. `get_key()`/`get_keys()` populate `has_password` from the wallet config. `import_key()` is unchanged — it still encrypts with `b""`.

**`offers.rs`**: `make_offer()`, `take_offer()` pass password to `extract_secrets()`. `cancel_offer()`, `cancel_offers()` pass password to `transact()`.

**`actions.rs`**: `increase_derivation_index()` passes password to `extract_secrets()`.

**`wallet_connect.rs`**: Both signing methods pass password to `extract_secrets()`.

New `change_password()` endpoint.

### Frontend (TypeScript/React)

#### PasswordContext (`src/contexts/PasswordContext.tsx`) — _(superseded)_

**As originally built**, this provider was the single entry point callers invoked:

```typescript
requestPassword(hasPassword: boolean, fingerprint?: number): Promise<string | null | undefined>;
```

`string` meant "use this password", `null` meant "no auth needed", `undefined` meant "cancelled —
abort". Its decision tree was:

```text
hasPassword=true                          → show password dialog (password always takes precedence)
hasPassword=false, biometric enabled      → biometric prompt with 5-min cache, return null on success, undefined on fail
hasPassword=false, biometric not enabled  → return null (no auth needed)
cancelled at any point                    → return undefined
```

**As it works now**, the provider is a responder, not a callable. It subscribes to the Rust
`PasswordRequest` event, runs that same three-way decision tree unchanged, and replies with
`submit_password_response(request_id, Password | NoAuthNeeded | Cancelled)`. Requests are queued by
`requestId`, so overlapping prompts cannot interleave. `requestPassword` is gone; the only export is
`requireLocalAuth()`, a UI-only biometric gate for the two `Settings.tsx` actions that touch no wallet
secret (starting the RPC server, toggling run-on-startup) and therefore have no Rust operation to hang
a prompt off.

The mutual-exclusivity rule, the 5-minute biometric cache, and the desktop fallback are all preserved
bit-for-bit. What changed is the direction of the call. See
[2026-08-21-password-gate-design.md](2026-08-21-password-gate-design.md).

**Provider placement:** Inside `I18nProvider` and `WalletProvider`. Wraps `WalletConnectProvider` and
all downstream providers.

Provider tree: `BiometricProvider` → `I18nProvider` → `WalletProvider` → `PasswordProvider` →
`PeerProvider` → `WalletConnectProvider` → `PriceProvider` → `RouterProvider`

#### PasswordDialog (`src/components/dialogs/PasswordDialog.tsx`)

A reusable modal dialog rendered by `PasswordProvider`. Unchanged by the password gate. Features:

- Auto-focuses the password input on open
- Clears password state on open/close
- Supports Enter key to submit
- Cancel resolves the prompt as cancelled

Retries are driven from Rust: a wrong password re-emits the same `requestId` with an incremented
`attempt` and the remaining-attempt count, up to three attempts.

#### usePassword hook (`src/hooks/usePassword.ts`)

Thin wrapper around `PasswordContext` with a guard that throws if used outside `PasswordProvider`.
`Settings.tsx` is its only consumer.

#### Call site pattern — _(superseded)_

Every protected operation used to open with:

```typescript
const password = await requestPassword(wallet?.has_password ?? false);
if (password === undefined) return; // auth cancelled or failed
```

**No call site does this any more.** `requestPassword` has no remaining references in `src/`. Call
sites simply invoke the command; if authentication is needed, Rust prompts before executing and
returns `Unauthorized` if it is refused. The files that carried the old plumbing —
`ConfirmationDialog.tsx`, `WalletCard.tsx`, `Settings.tsx` (`increaseDerivationIndex`), `Offers.tsx`,
`OfferRowCard.tsx`, `useOfferProcessor.ts`, `Offer.tsx`, and the WalletConnect command layer — were
all stripped.

#### WalletConnect integration — _(superseded)_

`HandlerContext` was extended with `requestPassword` and `hasPassword`, and each handler prompted
before executing. Both fields have since been removed from the handler context, and
`src/walletconnect/commands/{chip0002,high-level,offers}.ts` no longer prompt. Because these
handlers set `auto_submit: true`, the gate fires inside the Rust command.

#### Password management in Settings

A new **Security** section in Wallet Settings (only shown for hot wallets with `has_secrets`):

- **Set Password** — shown when `has_password` is `false`. Opens a dialog with New Password + Confirm Password fields.
- **Change Password** — shown when `has_password` is `true`. Opens a dialog with Current Password + New Password + Confirm Password fields.
- **Remove Password** — shown when `has_password` is `true`. Opens a dialog with Current Password field. Uses destructive button variant.

All three operations call `commands.changePassword()` with appropriate `old_password`/`new_password` values (empty string = no password). On success, refreshes `KeyInfo` via `commands.getKey()` and shows a success toast.

#### Error feedback

All password feedback is centralized in `ErrorContext.addError`, so no call site handles it:

| Error                                                   | Result                                                                                                       |
| ------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| `incorrect_password`                                    | "Incorrect password" toast, then a `reconcileKeyProtection` probe if the flag says the wallet is unprotected |
| `unauthorized` / "Password entry cancelled"             | Silent — a deliberate cancellation is not a failure                                                          |
| `unauthorized` / "Too many incorrect password attempts" | Translated toast                                                                                             |
| `unauthorized` / "Password prompt timed out"            | Translated toast                                                                                             |
| `unauthorized` containing "not found" / "No secret"     | Toast with the raw reason                                                                                    |
| other `unauthorized`                                    | Silent — `NotLoggedIn` / `NoSigningKey` during wallet transitions                                            |

The three gate reasons are matched on the exact Rust reason string and re-rendered as translated
text; the constants are duplicated in `ErrorContext.tsx` with comments naming their Rust source.

#### Settings UI changes

The biometric toggle remains in the **Preferences** section of Global Settings (not per-wallet Security) because it is a global setting that applies to all wallets. It is only visible on mobile when biometric hardware is available and enrolled.

#### Design decisions

- **No password at import time** — users set a password later via Settings. Simpler UX, same security outcome.
- **No session caching** — every protected operation prompts independently. Passwords are never stored on device.
- **Single dialog instance** — `PasswordProvider` renders one `PasswordDialog` at the provider level, avoiding duplicate dialog instances across components.
- **Unified auth entry point** — a single decision tree handles password and biometric, subsuming the standalone `promptIfEnabled()` check. `BiometricContext` continues to exist for state management (`enabled`, `available`) and to run the prompt, but nothing calls `promptIfEnabled()` at an operation site. Since the password gate, the entry point is a Rust event rather than a function call.
- **Mutual exclusivity** — biometric and password are mutually exclusive. Password takes precedence. If a wallet has a password, the password dialog is always shown regardless of biometric settings. Biometric is a standalone gate for no-password wallets only.
- **Global biometric setting** — one toggle applies to all wallets. No per-wallet biometric configuration needed.

## Error Handling

- **Wrong password**: AES-GCM authentication fails → `KeychainError::Decrypt` → frontend shows "Incorrect password" toast.
- **Public-key-only wallets**: `extract_secrets` returns `(None, None)` — no prompt needed. Frontend checks `has_secret_key` and `has_password` to decide.
- **Lost password**: No recovery. AES-256-GCM + Argon2 is irreversible without the password. UI warns at password-set time. Matches industry standard (Chia reference wallet, MetaMask).
- **Biometric lockout**: After too many failed OS-level biometric attempts, the OS locks biometric temporarily. Only affects no-password wallets using the biometric gate.
- **App backgrounded during biometric**: OS may cancel the biometric prompt. Treated as cancellation → `requestPassword` returns `undefined`.

## Migration

Existing keys encrypted with `b""` continue to work — the user simply never gets prompted. To add protection, the user triggers "Set Password" which calls `change_password(fingerprint, b"", new_password)`.

**No `keys.bin` migration is needed.** The has-password flag lives in the wallet config, which
defaults `password_protected` to `false`, so existing `config.toml` files deserialize unchanged and
existing `keys.bin` files are read by the same code as before. (An earlier draft put the flag in
`KeyData::Secret` and would have needed a versioned deserialization fallback; see
**Has-Password Indicator**.)

## What's NOT Changing

- `encrypt.rs` — AES-256-GCM + Argon2 implementation is already correct
- `keys.bin` encryption scheme — same Argon2 + AES-256-GCM, just with real passwords instead of `b""`
- Any sync, peer, or database logic
- `SendTransactionImmediately`, `SubmitTransaction`, `ViewCoinSpends` — these operate on pre-signed spend bundles or read-only data and do not call `extract_secrets()` or `sign()`
- Backend — no backend changes for the biometric gate. Rust never learns whether biometrics are enabled; it emits a prompt request and the frontend decides how to satisfy it.
- `keys.bin` format — `KeyData::Secret` is unchanged.
- Biometric — remains as a standalone gate for no-password wallets. `BiometricContext` provides `enabled`/`available` state; `PasswordContext` handles the actual biometric prompt internally.
