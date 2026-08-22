# Password Gate: Rust-Owned Password Prompting

**Date:** 2026-08-21
**Branch:** `password-gate` (off `password`)
**Status:** Approved design, ready for implementation planning

## Problem

Password prompting in Sage is decided by the frontend. `PasswordContext.requestPassword(hasPassword)`
reads the wallet's `password_protected` flag, chooses between a password dialog, a biometric gate, and
no auth at all, and then threads the resulting string into the command it is about to call. Roughly
sixteen call sites across React pages, components, hooks, and the WalletConnect command layer repeat
this pattern.

This has two consequences:

1. **Apps cannot transact on a protected wallet.** The sage-apps bridge hardcodes `password: None` in
   its request conversions (`send_xch.rs`, `sign_message.rs`, `sign_coin_spends.rs`), so any bridge
   request against a password-protected wallet fails to decrypt. Apps have no path to prompt, and
   giving them one would mean the master-key password passing through a sandboxed app webview.
2. **The decision is in the least trustworthy place.** Whether an operation requires authentication is
   a security property of the wallet, not a UI concern. Every new caller must remember to ask.

## Goals

- Rust decides when authentication is required. Callers never supply a password and never decide.
- The password dialog renders only in the trusted `main` webview. Secrets never enter app-land.
- Apps gain a working path to operate on protected wallets.
- All existing in-app password prompting is replumbed through the same choke point.

## Non-goals

- **Session unlock / key caching.** Prompting once and holding the decrypted master key in memory for
  a window is a separate change to the security model. Deferred.
- **`ChangePassword`.** Its `old_password` / `new_password` are password _management_ form data, not
  wallet unlocking. Unchanged.
- **`passkey-unlock`.** Independent work on its own branch.

## Key constraint: sage-rpc is headless

`sage-rpc` drives the same `Arc<Mutex<Sage>>` core over mTLS (`crates/sage-rpc/src/lib.rs:33`) and its
clients legitimately supply `password` in the request body (`crates/sage-rpc/src/tests.rs:216`). There
is no UI to prompt.

Therefore:

- `password: Option<String>` **stays** on the `sage-api` request types. It is the RPC contract.
- `Sage::sign(coin_spends, partial, &password)` and the `keychain.extract_secrets` call sites in
  `crates/sage/src/endpoints/` are **unchanged**.
- The prompting choke point lives in the **Tauri host layer, above `Sage`** — never in the core.

This placement also removes a re-entrancy hazard. Every endpoint runs inside `app_state.lock().await`.
Awaiting a round-trip to the main webview while holding that lock risks deadlock if the webview's
handler invokes any command needing the same lock. Resolving the password _before_ the lock is taken
avoids the problem entirely.

## Architecture

New crate `crates/sage-password-gate`. It lives outside `src-tauri/src/` because both `sage-tauri`
and `sage-apps` must call it, and `sage-apps` cannot depend on the `sage-tauri` binary crate. Its entry
point is
`resolve(app_handle, state) -> Result<Option<String>>`. It reads the active wallet's fingerprint and
`password_protected` flag from `state` without holding the lock across the round-trip, asks the main
webview, validates the answer against the keychain, and returns a verified password or `None`.

### Transport

Rust to the main webview is a tauri-specta event; the reply returns as a command, because a password
must not ride an event broadcast.

- **Event** `PasswordRequest { request_id, fingerprint, requires_password: bool, attempt: u8, error: Option<PasswordAttemptError> }`,
  emitted with `emit_to(SAGE_WEBVIEW_LABEL, ...)`. Never plain `emit` — no app webview may observe it.
- **Command** `submit_password_response(request_id, outcome)` where
  `outcome = Password(String) | NoAuthNeeded | Cancelled`.
- **State** `PasswordGateState { pending: Mutex<HashMap<String, oneshot::Sender<Outcome>>> }`.
  The gate awaits the oneshot.

`requires_password` is advisory rather than the whole decision. Rust knows `password_protected`; it
does not know whether biometric auth is enabled, which is a UI and plugin setting. So the gate always
emits, and the frontend keeps today's exact three-way logic: password dialog, biometric gate with its
existing five-minute cache, or an immediate `NoAuthNeeded`. Biometric logic stays where it belongs and
behavior is preserved bit-for-bit. The cost is one sub-millisecond IPC round-trip on unprotected
wallets.

_Rejected:_ pushing the biometric setting into Rust to skip that round-trip. Not worth the state-sync
complexity for the latency saved.

### Verification and retry

The gate verifies with `keychain.extract_secrets(fingerprint, &password)` **before** taking the app
lock. On `KeychainError::Decrypt` it re-emits with an incremented `attempt` and an inline error, up to
three attempts, then fails `Unauthorized`. `Cancelled` fails immediately with a distinct error kind so
the frontend can stay silent rather than surfacing a toast.

Verifying above the lock is what makes bounded retry cheap: a wrong password costs one keychain
decrypt, not a partially built transaction.

### Gating the endpoints

Every endpoint command is generated from a single `repeat` block (`src-tauri/src/commands.rs:67-75`)
driven by `crates/sage-api/endpoints.json`. The gate therefore goes in exactly one place.

Add a `maybe_unlock` token to `crates/sage-api/macro/src/lib.rs` alongside `maybe_async` and
`maybe_await`, driven by a new gated-endpoint set. The repeat block becomes:

```rust
pub async fn endpoint(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    gate: State<'_, PasswordGateState>,
    mut req: Endpoint,
) -> Result<EndpointResponse> {
    maybe_unlock;  // expands to: req.password = gate.resolve(&app_handle, &state).await?;
    Ok(state.lock().await.endpoint(req) maybe_await?)
}
```

`maybe_unlock` expands to nothing for ungated endpoints.

Drift is prevented by a `sage-api` test asserting the gated set is exactly the set of request types
carrying a `password` field. Adding a signing endpoint without gating it fails the build.

### Bridge path (apps)

The two dialogs are sequential. The user approves the request summary in the `bridge-approval` system
app exactly as today; the password is then collected in the main webview.

In `process_after_approval` (`crates/sage-apps/src/bridge/bridge_request.rs:83`), after `approved ==
true` and the wallet-binding check passes, and before `process_shared`:

1. Hide the `bridge-approval` runtime via `hide_runtime_inner` so it does not cover the main webview.
   App runtimes are sibling webviews inside the same `main` window (`runtime/manager.rs:395-414`), so
   a React dialog would otherwise render underneath.
2. Call the gate.
3. Execute.

Because approval and password are now two phases, the original `expires_at_ms` keeps running through
the prompt. A lapse mid-prompt fails with `approval_timeout`. One clock, no new concept, and an app
cannot hold a signing path open indefinitely.

The verified password is threaded into the handler through `BridgeTools`, so the conversions in
`send_xch.rs`, `sign_message.rs`, and `sign_coin_spends.rs` set `Some(...)` instead of the hardcoded
`None`.

Separately, `WalletSendXch::approval_request` stops returning `Ok(None)` on a protected wallet even
when `WalletSendXchAutoSubmit` is granted. A password-protected wallet always gets an approval; silent
auto-submit is incompatible with password protection.

### Frontend

`PasswordContext` inverts. It stops exporting `requestPassword` as something callers invoke and
instead subscribes to `PasswordRequest`, runs its existing three-way decision, and replies via
`submit_password_response`. `PasswordDialog` itself is unchanged.

All call sites then drop their password plumbing:

- `WalletCard.tsx`, `ConfirmationDialog.tsx`, `useOfferProcessor.ts`, `Offer.tsx` — remove the
  `requestPassword` call and the `password` field on the command.
- `src/walletconnect/` — `chip0002.ts`, `high-level.ts`, and `offers.ts` lose their prompts;
  `handler.ts` and `WalletConnectContext.tsx` drop `requestPassword` and `hasPassword` from the
  handler context entirely.
- `Settings.tsx:1106` and `:1123` gate starting the RPC server and toggling run-on-startup. No wallet
  secret is involved, so there is no Rust unlock operation to hang them off. They get a new,
  explicitly named `requireLocalAuth()` from the same provider: a UI-only biometric gate with no Rust
  round-trip.

## Error handling

| Condition                           | Result                                             |
| ----------------------------------- | -------------------------------------------------- |
| Correct password                    | Endpoint executes                                  |
| Wrong password, attempts 1-2        | Re-prompt with inline error, `attempt` incremented |
| Wrong password, attempt 3           | `Unauthorized`                                     |
| User cancels                        | Distinct cancellation error; frontend stays silent |
| Approval deadline lapses mid-prompt | `approval_timeout`, dialog closes                  |
| Main webview absent or unresponsive | Gate fails; operation does not proceed             |

## Testing

**Rust**

- Gate unit tests against a mock responder: correct password; wrong-then-right; three strikes;
  cancellation; expiry mid-prompt.
- The drift test asserting gated set == request types with a `password` field.
- A bridge test that a protected wallet forces an approval despite the `WalletSendXchAutoSubmit` grant.
- Existing `sage-rpc` password tests must pass **unchanged** — the regression canary proving the core
  was not disturbed.

**TypeScript**

The repository has no frontend test runner (no vitest or jest, no `test` script in `package.json`),
and bootstrapping one inside this feature is out of scope. Frontend changes are verified by
`pnpm run build:frontend` (`tsc -b`), `pnpm run lint`, and a manual smoke run covering all three
responder branches: password dialog on a protected wallet, biometric gate on an unprotected wallet
with biometrics on, and immediate `NoAuthNeeded` otherwise.

## Files touched

**New**

- `crates/sage-password-gate/` (`lib.rs`, `types.rs`, `prompter.rs`, `resolve.rs`)

**Rust**

- `src-tauri/src/commands.rs` — repeat block, `submit_password_response`
- `src-tauri/src/lib.rs` — register state, command, and event
- `src-tauri/src/error.rs` — `From<sage_password_gate::Error>`
- `crates/sage-apps/Cargo.toml`, `src-tauri/Cargo.toml`, root `Cargo.toml` — new crate wiring
- `crates/sage-api/macro/src/lib.rs` — `maybe_unlock`
- `crates/sage-api/endpoints.json` (or a sibling gated-endpoint set)
- `crates/sage-apps/src/bridge/bridge_request.rs` — gate call in `process_after_approval`
- `crates/sage-apps/src/bridge/methods/user/wallet/{send_xch,sign_message,sign_coin_spends}.rs`

**TypeScript**

- `src/contexts/PasswordContext.tsx`, `src/hooks/usePassword.ts`
- `src/contexts/WalletConnectContext.tsx`, `src/walletconnect/{handler,commands/chip0002,commands/high-level,commands/offers}.ts`
- `src/components/{WalletCard,ConfirmationDialog}.tsx`
- `src/hooks/useOfferProcessor.ts`, `src/pages/{Offer,Settings}.tsx`
- `src/bindings.ts` (regenerated)

**Unchanged, deliberately**

- `crates/sage/src/**` — all endpoints and `Sage::sign`
- `crates/sage-rpc/src/**`
- `password: Option<String>` on all `sage-api` request types
