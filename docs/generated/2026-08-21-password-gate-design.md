# Password Gate: Rust-Owned Password Prompting

**Date:** 2026-08-21
**Branch:** `password-gate` (off `password`)
**Status:** Implemented

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

A sibling entry point `resolve_for_fingerprint(app_handle, state, gate, fingerprint)` targets an
explicitly named wallet. Endpoints whose request type carries a `fingerprint` — `delete_key` and
`get_secret_key`, plus the `wallet.getSecretKey` bridge method — act on a wallet that need not be the
active one, and are driven from the logged-out wallet list where there is no active wallet at all.
`resolve` would both prompt for the wrong wallet's password and fail with `NotLoggedIn`, so those
endpoints use the fingerprint-targeted form. The `password_protected` lookup searches
`wallet_config.wallets` by fingerprint and needs no active wallet, so this path never calls
`Sage::wallet()`.

### Gating manifest

`crates/sage-api/password-gating.json` maps each password-bearing endpoint to one of three modes,
and the macro's `maybe_unlock` expands accordingly:

| Mode          | Expansion                                                                          | Applies to                                                                   |
| ------------- | ---------------------------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| `always`      | `resolve(...)` on every call                                                       | endpoints that reach a secret unconditionally                                |
| `fingerprint` | `resolve_for_fingerprint(..., req.fingerprint)` on every call                      | `delete_key`, `get_secret_key`                                               |
| `auto_submit` | `resolve(...)` only when `req.auto_submit` is set, `req.password = None` otherwise | endpoints that only forward the password to `Sage::transact`/`transact_with` |

The `auto_submit` mode exists because those endpoints build a transaction for the confirmation
dialog first and touch no key until the caller asks for it to be signed and submitted. Prompting
unconditionally there asks the user for a password that is then discarded, and then asks again after
they confirm — two dialogs for one send.

Note that carrying an `auto_submit` field is _not_ the criterion: `sign_coin_spends` and `take_offer`
both have one but reach the keychain on every call, so both are `always`. The criterion is how the
implementation consumes the password, and a drift test in `crates/sage-api/src/lib.rs` enforces
exactly that by scanning `crates/sage/src/endpoints/` — an endpoint whose body calls
`extract_secrets` or `self.sign` must not be `auto_submit`, and one that only forwards to
`transact`/`transact_with` must be. Sibling tests keep the manifest's key set equal to the set of
request types carrying a `password` field, keep `fingerprint` equal to those carrying a
`fingerprint` field, and reject unknown mode strings.

### Transport

Rust to the main webview is a tauri-specta event; the reply returns as a command, because a password
must not ride an event broadcast.

- **Event** `PasswordRequest { request_id, requires_password: bool, attempt: u8, error: Option<PasswordAttemptError> }`,
  emitted with `emit_to(SAGE_WEBVIEW_LABEL, ...)` rather than a plain `emit`.

  Targeting `main` is where the request is _meant_ to land, not a guarantee of where it _can_ land.
  Tauri resolves an `AnyLabel` target through `Listeners::emit_js_filter` → `match_any_or_filter`,
  which short-circuits to true for any listener registered with `EventTarget::Any`, never consulting
  the target. `src-tauri/capabilities/apps.json` grants app webviews `core:event:allow-listen`, and
  `plugin:event|listen` takes its target straight from JS, so an app runtime that calls
  `listen('password-request', …)` receives every prompt. The payload is therefore designed to be
  observable: it carries no wallet fingerprint, no wallet identity, and no password. `attempt` and
  `error.attemptsRemaining` are retained because the dialog needs them and a bare retry counter
  identifies nothing an observer could not already infer from the re-prompt timing itself.

  What keeps the _password_ safe is direction, not targeting: the secret only ever travels back on
  `submit_password_response`, a command absent from both `apps.json` and `system-apps.json`, and apps
  hold no `core:event:allow-emit` with which to forge a request.

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

Every endpoint command is generated from a single `repeat` block (`src-tauri/src/commands.rs:68-83`)
driven by `crates/sage-api/endpoints.json`. The gate therefore goes in exactly one place.

A `maybe_unlock` token in `crates/sage-api/macro/src/lib.rs` sits alongside `maybe_async` and
`maybe_await`, driven by the gating manifest above. The repeat block is:

```rust
#[command]
#[specta]
#[allow(unused_variables, unused_mut)]
pub async fn endpoint(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    gate: State<'_, PasswordGateState>,
    mut req: Endpoint,
) -> Result<EndpointResponse> {
    maybe_unlock
    Ok(state.lock().await.endpoint(req) maybe_await?)
}
```

`maybe_unlock` expands to nothing for endpoints absent from the manifest, so the ~91 ungated
commands are byte-identical to what they were before the gate existed. The `#[allow]` covers those
expansions, where `app_handle`, `gate`, and `mut` all go unused.

### Bridge path (apps)

There is one dialog, not two. The `bridge-approval` system app collects the password inline: summary
and password field in the same card, approved in one gesture. The main-window prompt is not used on
this path, so nothing has to hide the approval runtime to uncover a dialog underneath it, and the
hide/restore flicker that arrangement required is gone.

`PendingBridgeApprovalView` carries `requiresPassword`, computed when the approval is queued, and the
card renders its field from that. The flag is a hint, not the authority.

`process_after_approval` (`crates/sage-apps/src/bridge/bridge_request.rs:86`) **peeks** the pending
approval rather than consuming it, so a wrong password can leave it queued:

1. Every rejection that does not depend on the password — denied, expired, wallet re-bound — resolves
   first, so the user is never asked to type a password into a doomed approval.
2. `approval_needs_password` re-derives, at resolve time, whether this body reaches a secret and
   whether the wallet it targets is protected. A card whose hint was stale gets `passwordRequired`
   back and shows the field.
3. The candidate is verified against the keychain — the fingerprint in the body for `GetSecretKey`,
   the active wallet otherwise.
4. Wrong password: `password_attempts` on the host record increments and the card gets
   `wrongPassword { attemptsRemaining }`. The approval stays queued and the app keeps waiting. The
   counter lives on the host, so reloading the approval webview cannot reset it.
5. `MAX_ATTEMPTS` spent: the approval is consumed, the app's request fails `unauthorized` with
   `TOO_MANY_ATTEMPTS_REASON`, and the card gets `tooManyAttempts`. Both paths share the constant
   with the native prompt, so the two give the same number of tries.
6. Verified: the approval is consumed and the password goes straight into `process_shared`.

An approval that needs a password is queued with `BRIDGE_APPROVAL_PASSWORD_TIMEOUT_MS` (3 minutes)
instead of the usual 30 seconds: reading a summary, typing a master password, and recovering from a
typo does not fit in 30 seconds. The deadline is still a single clock an app cannot extend.

`ResolveBridgeApprovalArgs` carries the password and a hand-written `Debug` that prints it as
`Some("<redacted>")`, matching `BridgeTools`. The value is never written to the approval record and
never crosses back into an app runtime.

Only the four approval bodies that reach a wallet secret are gated — `GetSecretKey`, `SendXch`,
`SignCoinSpends`, `SignMessage`. `approval_requires_password` matches on the body exhaustively, with
no catch-all, so `CapabilityGrant` and `NetworkWhitelistGrant` prompt for nothing and a new body must
opt in deliberately.

The verified password is threaded into the handler through `BridgeTools`, so the conversions in
`send_xch.rs`, `sign_message.rs`, `sign_coin_spends.rs`, and `get_secret_key.rs` set `Some(...)`
instead of the hardcoded `None`. `BridgeTools` carries a hand-written `Debug` that prints the field
as `Some("<redacted>")`.

Collecting the password in the `bridge-approval` webview rather than the main one is a deliberate
trust-boundary choice: that webview is host-owned system UI, never a third-party app runtime, and the
value travels the system bridge straight to the host. Apps are desktop-only, so nothing is lost by
this path no longer consulting the frontend's biometric gate, which only ever applied on mobile.

The bridge does not go through the endpoint macro, so the gating manifest does not apply to it. Both
`send_xch` and `sign_coin_spends` reach a secret on every bridge call — the former hardcodes
`auto_submit: true`, the latter signs unconditionally — so the bridge gate is unconditional too.

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
- Two sites in `Settings.tsx` gate starting the RPC server and toggling run-on-startup. No wallet
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
  cancellation; prompt timeout.
- The drift tests over `password-gating.json` described under **Gating manifest**: key set ==
  request types with a `password` field, `fingerprint` mode == those with a `fingerprint` field,
  `auto_submit` mode == endpoints that only forward the password to `transact`/`transact_with`.
- A bridge test that a protected wallet forces an approval despite the `WalletSendXchAutoSubmit` grant.
- Bridge attempt-accounting tests: a wrong password reports the attempts left, the last one exhausts
  the approval, over-counting still fails closed, and only secret-bearing bodies require a password
  (with `GetSecretKey` targeting its own fingerprint).
- Drift-reconciliation tests in `sage-rpc`: `login` re-derives a flag that was forced false, and
  `reconcile_all_key_protection` corrects every wallet in both directions and is idempotent.
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
- `crates/sage-api/macro/src/lib.rs` — `maybe_unlock` and `GateMode`
- `crates/sage-api/password-gating.json` — the gating manifest
- `crates/sage-api/src/lib.rs` — the drift tests
- `crates/sage-apps/src/bridge/bridge_request.rs` — gate call in `process_after_approval`
- `crates/sage-apps/src/bridge/methods/user/wallet/{send_xch,sign_message,sign_coin_spends,get_secret_key}.rs`
- `crates/sage-apps/src/bridge/methods/shared.rs` — `BridgeTools.password`
- `src-tauri/permissions/main-host.toml` — `submit_password_response` in `commands.allow`

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
