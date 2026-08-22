# Password Gate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move the decision of _when_ a password is required out of the frontend and into the Rust
Tauri host, so callers never supply a password, and app-bridge requests can operate on
password-protected wallets with the dialog rendering only in the trusted `main` webview.

**Architecture:** A new `password_gate` module in the Tauri host sits above `Sage` and above
`app_state.lock()`. It emits a `PasswordRequest` event to the `main` webview only, awaits a reply
delivered back through a `submit_password_response` command, verifies the answer against the keychain,
and hands the verified password down through the existing `password: Option<String>` API field. The
`sage` and `sage-rpc` crates are not touched — `sage-rpc` is headless and its clients legitimately
supply passwords in the request body.

**Tech Stack:** Rust, Tauri 2, tauri-specta, tokio (`oneshot`, `Mutex`), proc-macro (`sage-api-macro`),
React 18 + TypeScript, pnpm, Vite.

**Spec:** `docs/superpowers/specs/2026-08-21-password-gate-design.md`

## Global Constraints

- **Branch:** `password-gate`, off `password`. Do not merge to `main`.
- **Commit per task on `password-gate` only.** The user has authorised commits on this branch for the
  duration of this plan. Never push, never merge, never commit on `main` or `password`. The standing
  "no auto-commit" preference still applies everywhere outside this branch.
- **Every `cargo` command requires `export SDKROOT="$(xcrun --show-sdk-path)"` first**, or the build
  fails with `stdlib.h not found`.
- **Never run `pnpm run extract`** (lingui). `.po` churn is batched into a separate pre-release pass.
- **Do not modify `crates/sage/**`or`crates/sage-rpc/**`.** Not `Sage::sign`, not any endpoint in
  `crates/sage/src/endpoints/`. If a task seems to need this, stop and report.
- **Do not remove `password: Option<String>` from any `sage-api` request type.** It is the RPC contract.
- **`ChangePassword` is out of scope.** Its `old_password` / `new_password` stay caller-supplied.
- **Regenerate bindings with `pnpm run generate:bindings`**, never by hand-editing `src/bindings.ts`.
- The exact set of 32 password-gated endpoints is listed in Task 3. It is 1:1 with the `sage-api`
  request types carrying a `password` field.

---

## File Structure

**New files**

| File                                        | Responsibility                                                        |
| ------------------------------------------- | --------------------------------------------------------------------- |
| `crates/sage-password-gate/Cargo.toml`      | New crate manifest                                                    |
| `crates/sage-password-gate/src/lib.rs`      | Public surface: `PasswordGateState`, `resolve()`, `Error`, re-exports |
| `crates/sage-password-gate/src/types.rs`    | `PasswordRequest` event, `PasswordOutcome`, `PasswordAttemptError`    |
| `crates/sage-password-gate/src/prompter.rs` | `Prompter` trait + `TauriPrompter` (emit to `main` webview)           |
| `crates/sage-password-gate/src/resolve.rs`  | Verify-and-retry loop; unit-tested against a mock `Prompter`          |

The gate lives in its own crate rather than in `src-tauri/src/` because **both** `sage-tauri` and
`sage-apps` must call it — `sage-apps` cannot depend on the `sage-tauri` binary crate. Putting it here
from the start avoids extracting it later. It depends only on `sage`, `sage-api`, `sage-keychain`, and
`tauri`; nothing depends on it except the two hosts.

Splitting the gate across four small files keeps the testable core (`resolve.rs`) free of any Tauri
`AppHandle` dependency. `resolve.rs` talks only to the `Prompter` trait, so its tests need no running
Tauri app. This is the single most important structural decision in the plan — do not collapse these
into one file.

**Modified files**

| File                                                                                                                            | Change                                                      |
| ------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------- |
| `crates/sage-api/macro/src/lib.rs`                                                                                              | Add `maybe_unlock` token expansion                          |
| `crates/sage-api/password-gated.json`                                                                                           | New: the 32 gated endpoint names                            |
| `crates/sage-api/src/lib.rs`                                                                                                    | Drift test module                                           |
| `src-tauri/src/commands.rs`                                                                                                     | Repeat block gains the gate; new `submit_password_response` |
| `src-tauri/src/lib.rs`                                                                                                          | Register state, command, event                              |
| `crates/sage-apps/src/bridge/bridge_request.rs`                                                                                 | Gate call in `process_after_approval`                       |
| `crates/sage-apps/src/bridge/types.rs`                                                                                          | `BridgeTools` carries the resolved password                 |
| `crates/sage-apps/src/bridge/methods/user/wallet/{send_xch,sign_message,sign_coin_spends}.rs`                                   | Use the resolved password; force approval when protected    |
| `src/contexts/PasswordContext.tsx`                                                                                              | Invert to a responder; add `requireLocalAuth`               |
| `src/hooks/usePassword.ts`                                                                                                      | Export the new shape                                        |
| `src/pages/Settings.tsx`                                                                                                        | Use `requireLocalAuth`                                      |
| `src/components/{WalletCard,ConfirmationDialog}.tsx`, `src/hooks/useOfferProcessor.ts`, `src/pages/Offer.tsx`                   | Drop password plumbing                                      |
| `src/contexts/WalletConnectContext.tsx`, `src/walletconnect/{handler,commands/chip0002,commands/high-level,commands/offers}.ts` | Drop password plumbing                                      |
| `src/bindings.ts`                                                                                                               | Regenerated                                                 |

---

## Task 1: Gate types and the testable resolve loop

This task builds the entire decision core with **no Tauri dependency**, so it is fully unit-testable.

**Files:**

- Create: `crates/sage-password-gate/Cargo.toml`
- Create: `crates/sage-password-gate/src/lib.rs`
- Create: `crates/sage-password-gate/src/types.rs`
- Create: `crates/sage-password-gate/src/prompter.rs`
- Create: `crates/sage-password-gate/src/resolve.rs` (tests live in a `#[cfg(test)] mod tests` here)
- Modify: `Cargo.toml` (workspace members + dependency entry)

**Interfaces:**

- Consumes: `sage_keychain::{Keychain, KeychainError}`, `sage_api::ErrorKind`, `sage::Sage`
- Produces:
  - `pub enum PasswordOutcome { Password { password: String }, NoAuthNeeded, Cancelled }` (serde `tag = "kind"`, snake_case)
  - `pub struct PasswordAttemptError { pub attempts_remaining: u8 }`
  - `pub struct PasswordRequest { pub request_id: String, pub fingerprint: u32, pub requires_password: bool, pub attempt: u8, pub error: Option<PasswordAttemptError> }`
  - `#[async_trait] pub trait Prompter { async fn prompt(&self, request: PasswordRequest) -> Result<PasswordOutcome>; }`
  - `#[async_trait] pub trait PasswordVerifier { async fn verify(&self, fingerprint: u32, password: &str) -> Result<bool>; }` — `Ok(false)` means wrong password; `Err` means a real failure
  - `pub struct Error { pub kind: ErrorKind, pub reason: String }` and `pub type Result<T> = std::result::Result<T, Error>` — the crate's own error, structurally identical to `src-tauri`'s so `From` is trivial
  - `pub const MAX_ATTEMPTS: u8 = 3;`
  - `pub const CANCELLED_REASON: &str = "Password entry cancelled";`
  - `pub async fn resolve_with(prompter: &dyn Prompter, verifier: &dyn PasswordVerifier, fingerprint: u32, requires_password: bool) -> Result<Option<String>>`

- [ ] **Step 1: Create the crate**

Create `crates/sage-password-gate/Cargo.toml`:

```toml
[package]
name = "sage-password-gate"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
sage = { workspace = true }
sage-api = { workspace = true, features = ["tauri"] }
sage-keychain = { workspace = true }
async-trait = "0.1.89"
serde = { workspace = true, features = ["derive"] }
specta = { workspace = true }
tauri = { workspace = true }
tauri-specta = { workspace = true }
tokio = { workspace = true, features = ["sync"] }
uuid = { version = "1.19.0", features = ["v4"] }

[dev-dependencies]
bip39 = { workspace = true }
tokio = { workspace = true, features = ["macros", "rt-multi-thread", "sync"] }
```

`async-trait` and `uuid` are **not** in the root `[workspace.dependencies]` table — `crates/sage-apps`
pins them locally, and the versions above match it exactly. Do not change them to `workspace = true`.
Copy the `version`/`edition`/`license` spellings and the `workspace = true` dependency forms from
`crates/sage-apps/Cargo.toml`.

Register the crate in the root `Cargo.toml`: add `"crates/sage-password-gate"` to `[workspace] members`
(if members are globbed as `crates/*`, no change is needed), and add
`sage-password-gate = { path = "./crates/sage-password-gate" }` to `[workspace.dependencies]`.

Create `crates/sage-password-gate/src/lib.rs`:

```rust
mod prompter;
mod resolve;
mod types;

use sage_api::ErrorKind;

pub use prompter::{PasswordVerifier, Prompter};
pub use resolve::{CANCELLED_REASON, MAX_ATTEMPTS, resolve_with};
pub use types::{PasswordAttemptError, PasswordOutcome, PasswordRequest};

/// This crate's error. Structurally identical to `sage-tauri`'s `Error`, so the
/// host converts with a trivial `From` impl.
#[derive(Debug, Clone)]
pub struct Error {
    pub kind: ErrorKind,
    pub reason: String,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.reason)
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;
```

Add to `src-tauri/src/error.rs` so host commands can use `?` on gate calls:

```rust
impl From<sage_password_gate::Error> for Error {
    fn from(error: sage_password_gate::Error) -> Self {
        Self { kind: error.kind, reason: error.reason }
    }
}
```

and add `sage-password-gate = { workspace = true }` to `src-tauri/Cargo.toml` dependencies. In
`src-tauri/src/lib.rs`, alias it for the shorter call sites used later:
`use sage_password_gate as password_gate;`

- [ ] **Step 2: Write the types**

Create `crates/sage-password-gate/src/types.rs`:

```rust
use serde::{Deserialize, Serialize};
use specta::Type;

/// How the frontend answered a password request.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PasswordOutcome {
    /// The user supplied a password.
    Password { password: String },
    /// No authentication was required, or a biometric gate already passed.
    NoAuthNeeded,
    /// The user dismissed the prompt.
    Cancelled,
}

/// Attached to a re-prompt after an incorrect password.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PasswordAttemptError {
    pub attempts_remaining: u8,
}

/// Emitted to the `main` webview only. Never broadcast.
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct PasswordRequest {
    pub request_id: String,
    pub fingerprint: u32,
    /// Advisory: the wallet's stored `password_protected` flag. The frontend
    /// still decides between password dialog, biometric gate, and no auth,
    /// because Rust does not know whether biometrics are enabled.
    pub requires_password: bool,
    /// 1-based. Increments on each incorrect-password re-prompt.
    pub attempt: u8,
    pub error: Option<PasswordAttemptError>,
}
```

- [ ] **Step 3: Write the Prompter trait**

Create `crates/sage-password-gate/src/prompter.rs`:

```rust
use async_trait::async_trait;

use super::types::{PasswordOutcome, PasswordRequest};
use crate::Result;

/// Abstracts the round-trip to the frontend so the resolve loop can be
/// unit-tested without a running Tauri app.
#[async_trait]
pub trait Prompter: Send + Sync {
    async fn prompt(&self, request: PasswordRequest) -> Result<PasswordOutcome>;
}

/// Checks a candidate password against the keychain.
///
/// This is a trait rather than a borrowed `&Keychain` on purpose. `Keychain`
/// is not `Clone` and owns a `ChaCha20Rng`; cloning it to escape the app lock
/// would duplicate an RNG stream, which is a nonce-reuse hazard the moment a
/// clone ever encrypts. Instead the production implementation takes the Sage
/// lock briefly for each attempt and drops it before the next prompt, so no
/// lock is ever held across an await.
#[async_trait]
pub trait PasswordVerifier: Send + Sync {
    /// `Ok(true)` = correct, `Ok(false)` = wrong password,
    /// `Err` = a genuine failure (not a wrong password).
    async fn verify(&self, fingerprint: u32, password: &str) -> Result<bool>;
}
```

`async-trait` was declared in the crate manifest in Step 1 with an explicit version, matching
`crates/sage-apps`. It is not a workspace dependency.

- [ ] **Step 4: Write the failing tests**

Create `crates/sage-password-gate/src/resolve.rs` with **only** this test module for now (the file will
not compile yet — that is expected and is the point of the next step):

```rust
#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use async_trait::async_trait;
    use sage_api::ErrorKind;
    use sage_keychain::{Keychain, KeychainError};

    use super::*;
    use crate::types::{PasswordOutcome, PasswordRequest};
    use crate::{Error, PasswordVerifier, Prompter, Result};

    /// Replays a scripted sequence of outcomes and records what it was asked.
    struct MockPrompter {
        scripted: Mutex<Vec<PasswordOutcome>>,
        seen: Mutex<Vec<PasswordRequest>>,
    }

    impl MockPrompter {
        fn new(scripted: Vec<PasswordOutcome>) -> Self {
            Self {
                scripted: Mutex::new(scripted.into_iter().rev().collect()),
                seen: Mutex::new(Vec::new()),
            }
        }

        fn seen(&self) -> Vec<PasswordRequest> {
            self.seen.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl Prompter for MockPrompter {
        async fn prompt(&self, request: PasswordRequest) -> Result<PasswordOutcome> {
            self.seen.lock().unwrap().push(request);
            Ok(self.scripted.lock().unwrap().pop().expect("prompted more times than scripted"))
        }
    }

    /// A verifier backed by a real keychain holding one mnemonic key
    /// encrypted with `password`.
    struct KeychainVerifier {
        keychain: Keychain,
    }

    #[async_trait]
    impl PasswordVerifier for KeychainVerifier {
        async fn verify(&self, fingerprint: u32, password: &str) -> Result<bool> {
            match self.keychain.extract_secrets(fingerprint, password.as_bytes()) {
                Ok(_) => Ok(true),
                Err(KeychainError::Decrypt) => Ok(false),
                Err(err) => Err(Error { kind: ErrorKind::Internal, reason: err.to_string() }),
            }
        }
    }

    fn protected_keychain(password: &str) -> (KeychainVerifier, u32) {
        let mut keychain = Keychain::default();
        let mnemonic = bip39::Mnemonic::from_entropy(&[7u8; 32]).unwrap();
        let fingerprint = keychain
            .add_mnemonic(&mnemonic, password.as_bytes())
            .expect("failed to add mnemonic");
        (KeychainVerifier { keychain }, fingerprint)
    }

    fn pw(s: &str) -> PasswordOutcome {
        PasswordOutcome::Password { password: s.to_string() }
    }

    #[tokio::test]
    async fn correct_password_resolves_on_first_attempt() {
        let (verifier, fingerprint) = protected_keychain("hunter2");
        let prompter = MockPrompter::new(vec![pw("hunter2")]);

        let result = resolve_with(&prompter, &verifier, fingerprint, true).await.unwrap();

        assert_eq!(result, Some("hunter2".to_string()));
        let seen = prompter.seen();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].attempt, 1);
        assert!(seen[0].requires_password);
        assert!(seen[0].error.is_none());
    }

    #[tokio::test]
    async fn wrong_then_right_reprompts_with_attempts_remaining() {
        let (verifier, fingerprint) = protected_keychain("hunter2");
        let prompter = MockPrompter::new(vec![pw("wrong"), pw("hunter2")]);

        let result = resolve_with(&prompter, &verifier, fingerprint, true).await.unwrap();

        assert_eq!(result, Some("hunter2".to_string()));
        let seen = prompter.seen();
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[1].attempt, 2);
        assert_eq!(seen[1].error.as_ref().unwrap().attempts_remaining, 2);
    }

    #[tokio::test]
    async fn three_wrong_attempts_fails_unauthorized() {
        let (verifier, fingerprint) = protected_keychain("hunter2");
        let prompter = MockPrompter::new(vec![pw("a"), pw("b"), pw("c")]);

        let error = resolve_with(&prompter, &verifier, fingerprint, true).await.unwrap_err();

        assert!(matches!(error.kind, ErrorKind::Unauthorized));
        assert_eq!(prompter.seen().len(), MAX_ATTEMPTS as usize);
    }

    #[tokio::test]
    async fn cancellation_fails_immediately_without_reprompting() {
        let (verifier, fingerprint) = protected_keychain("hunter2");
        let prompter = MockPrompter::new(vec![PasswordOutcome::Cancelled]);

        let error = resolve_with(&prompter, &verifier, fingerprint, true).await.unwrap_err();

        assert!(matches!(error.kind, ErrorKind::Unauthorized));
        assert_eq!(error.reason, CANCELLED_REASON);
        assert_eq!(prompter.seen().len(), 1);
    }

    #[tokio::test]
    async fn no_auth_needed_resolves_to_none() {
        let (verifier, fingerprint) = protected_keychain("hunter2");
        let prompter = MockPrompter::new(vec![PasswordOutcome::NoAuthNeeded]);

        let result = resolve_with(&prompter, &verifier, fingerprint, false).await.unwrap();

        assert_eq!(result, None);
        assert_eq!(prompter.seen().len(), 1);
        assert!(!prompter.seen()[0].requires_password);
    }
}
```

- [ ] **Step 5: Run the tests to verify they fail**

```bash
export SDKROOT="$(xcrun --show-sdk-path)"
cargo test -p sage-password-gate
```

Expected: FAIL to compile, with errors like `cannot find function 'resolve_with' in this scope` and
`cannot find value 'CANCELLED_REASON' in this scope`.

- [ ] **Step 6: Write the resolve loop**

Prepend to `crates/sage-password-gate/src/resolve.rs`, above the test module:

```rust
use sage_api::ErrorKind;

use super::types::{PasswordAttemptError, PasswordOutcome, PasswordRequest};
use crate::{Error, Result};

/// Maximum password entry attempts before the operation is refused.
pub const MAX_ATTEMPTS: u8 = 3;

/// Reason string used when the user dismisses the prompt, so the frontend can
/// distinguish a deliberate cancel from a genuine auth failure and stay silent.
pub const CANCELLED_REASON: &str = "Password entry cancelled";

fn unauthorized(reason: &str) -> Error {
    Error { kind: ErrorKind::Unauthorized, reason: reason.to_string() }
}

/// Prompts the frontend for a password and verifies it against the keychain,
/// retrying up to `MAX_ATTEMPTS` times on an incorrect password.
///
/// Returns `Ok(None)` when no authentication was required. The caller places
/// the returned value directly into the request's `password` field.
///
/// This runs *before* `app_state.lock()` is taken. Verifying here keeps a wrong
/// password cheap (one keychain decrypt, no partially built transaction) and
/// avoids awaiting the frontend while holding the app lock.
pub async fn resolve_with(
    prompter: &dyn Prompter,
    verifier: &dyn PasswordVerifier,
    fingerprint: u32,
    requires_password: bool,
) -> Result<Option<String>> {
    let mut error: Option<PasswordAttemptError> = None;

    for attempt in 1..=MAX_ATTEMPTS {
        let request = PasswordRequest {
            request_id: uuid::Uuid::new_v4().to_string(),
            fingerprint,
            requires_password,
            attempt,
            error: error.take(),
        };

        match prompter.prompt(request).await? {
            PasswordOutcome::NoAuthNeeded => return Ok(None),
            PasswordOutcome::Cancelled => return Err(unauthorized(CANCELLED_REASON)),
            PasswordOutcome::Password { password } => {
                if verifier.verify(fingerprint, &password).await? {
                    return Ok(Some(password));
                }
                error = Some(PasswordAttemptError {
                    attempts_remaining: MAX_ATTEMPTS - attempt,
                });
            }
        }
    }

    Err(unauthorized("Too many incorrect password attempts"))
}
```

Both `uuid` and `bip39` were declared in the crate manifest in Step 1 — `uuid` with an explicit
version (it is not a workspace dependency), `bip39` as `workspace = true`.

- [ ] **Step 7: Run the tests to verify they pass**

```bash
export SDKROOT="$(xcrun --show-sdk-path)"
cargo test -p sage-password-gate
```

Expected: PASS, 5 tests.

- [ ] **Step 8: Commit (only with user approval)**

```bash
git add crates/sage-password-gate Cargo.toml src-tauri/Cargo.toml src-tauri/src/error.rs src-tauri/src/lib.rs
git commit -m "feat(password-gate): add testable password resolve loop"
```

---

## Task 2: Tauri transport — state, event emission, response command

Wires the abstract `Prompter` to the real `main` webview.

**Files:**

- Modify: `crates/sage-password-gate/src/prompter.rs` (add `TauriPrompter`)
- Modify: `crates/sage-password-gate/src/lib.rs` (add `PasswordGateState`, `resolve`)
- Modify: `src-tauri/src/commands.rs` (add `submit_password_response`)
- Modify: `src-tauri/src/lib.rs` (register state, command, event)

**Interfaces:**

- Consumes: `Prompter`, `PasswordOutcome`, `PasswordRequest`, `resolve_with`, `MAX_ATTEMPTS` from Task 1
- Produces:
  - `pub struct PasswordGateState { pending: Mutex<HashMap<String, oneshot::Sender<PasswordOutcome>>> }` with `Default`
  - `pub async fn resolve(app_handle: &AppHandle, state: &AppState, gate: &PasswordGateState) -> Result<Option<String>>`
  - `pub fn submit_password_response(gate: State<'_, PasswordGateState>, request_id: String, outcome: PasswordOutcome) -> Result<()>`
  - Constant `SAGE_WEBVIEW_LABEL: &str = "main"`

- [ ] **Step 1: Write the failing test for the pending-map handoff**

Append to `crates/sage-password-gate/src/lib.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn resolving_a_pending_request_delivers_the_outcome() {
        let gate = PasswordGateState::default();
        let (tx, rx) = tokio::sync::oneshot::channel();
        gate.register("req-1".to_string(), tx).await;

        gate.deliver("req-1", PasswordOutcome::NoAuthNeeded)
            .expect("delivery should succeed");

        assert!(matches!(rx.await.unwrap(), PasswordOutcome::NoAuthNeeded));
    }

    #[tokio::test]
    async fn delivering_an_unknown_request_id_is_an_error() {
        let gate = PasswordGateState::default();

        let error = gate
            .deliver("nope", PasswordOutcome::Cancelled)
            .expect_err("unknown id must error");

        assert!(error.reason.contains("nope"));
    }

    #[tokio::test]
    async fn a_request_id_can_only_be_delivered_once() {
        let gate = PasswordGateState::default();
        let (tx, _rx) = tokio::sync::oneshot::channel();
        gate.register("req-2".to_string(), tx).await;

        gate.deliver("req-2", PasswordOutcome::Cancelled).unwrap();

        assert!(gate.deliver("req-2", PasswordOutcome::Cancelled).is_err());
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
export SDKROOT="$(xcrun --show-sdk-path)"
cargo test -p sage-password-gate
```

Expected: FAIL to compile — `cannot find type 'PasswordGateState'`, `no method named 'register'`.

- [ ] **Step 3: Implement the state**

Add to `crates/sage-password-gate/src/lib.rs`, above the test module:

```rust
use std::collections::HashMap;

use tokio::sync::{Mutex, oneshot};

/// Tracks in-flight password requests awaiting a frontend reply.
#[derive(Default)]
pub struct PasswordGateState {
    pending: Mutex<HashMap<String, oneshot::Sender<PasswordOutcome>>>,
}

impl PasswordGateState {
    pub(crate) async fn register(&self, request_id: String, tx: oneshot::Sender<PasswordOutcome>) {
        self.pending.lock().await.insert(request_id, tx);
    }

    pub(crate) async fn cancel(&self, request_id: &str) {
        self.pending.lock().await.remove(request_id);
    }

    /// Hands an outcome to the waiting resolve loop. Consumes the entry, so a
    /// given request id can only be answered once.
    pub(crate) fn deliver(&self, request_id: &str, outcome: PasswordOutcome) -> Result<()> {
        let sender = self
            .pending
            .blocking_lock()
            .remove(request_id)
            .ok_or_else(|| Error {
                kind: ErrorKind::NotFound,
                reason: format!("no pending password request with id {request_id}"),
            })?;

        sender.send(outcome).map_err(|_| Error {
            kind: ErrorKind::Internal,
            reason: "password request was abandoned".to_string(),
        })
    }
}
```

If `blocking_lock` panics in an async context during the tests, change `deliver` to `async fn` using
`self.pending.lock().await`, and make `submit_password_response` in Step 5 `async` to match. Prefer the
async form if in doubt — it is the safer choice inside Tauri's async command runtime.

- [ ] **Step 4: Run the tests to verify they pass**

```bash
export SDKROOT="$(xcrun --show-sdk-path)"
cargo test -p sage-password-gate
```

Expected: PASS, 8 tests.

- [ ] **Step 5: Implement `TauriPrompter` and the public `resolve`**

Append to `crates/sage-password-gate/src/prompter.rs`:

```rust
use tauri::{AppHandle, Emitter};
use tauri_specta::Event;

use super::{Error, PasswordGateState};
use sage_api::ErrorKind;

/// The Sage React webview. App runtimes are sibling webviews in the same
/// window, so emission MUST target this label — a plain `emit` would deliver
/// the request to app-land.
pub const SAGE_WEBVIEW_LABEL: &str = "main";

pub struct TauriPrompter<'a> {
    pub app_handle: &'a AppHandle,
    pub gate: &'a PasswordGateState,
}

#[async_trait]
impl Prompter for TauriPrompter<'_> {
    async fn prompt(&self, request: PasswordRequest) -> Result<PasswordOutcome> {
        let request_id = request.request_id.clone();
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.gate.register(request_id.clone(), tx).await;

        if let Err(err) = request.emit_to(self.app_handle, SAGE_WEBVIEW_LABEL) {
            self.gate.cancel(&request_id).await;
            return Err(Error {
                kind: ErrorKind::Internal,
                reason: format!("failed to emit password request: {err}"),
            });
        }

        rx.await.map_err(|_| Error {
            kind: ErrorKind::Internal,
            reason: "password request channel closed".to_string(),
        })
    }
}
```

If `tauri_specta::Event` does not provide `emit_to` in this version, use
`self.app_handle.emit_to(SAGE_WEBVIEW_LABEL, "password-request", &request)` instead and declare the
event name explicitly. Verify against the `SyncEvent` usage in `src-tauri/src/app_state.rs:52`.

Append to `crates/sage-password-gate/src/lib.rs`:

```rust
use std::sync::Arc;

use sage::Sage;
use tauri::AppHandle;

/// The host's shared Sage handle. Mirrors `sage_tauri::app_state::AppState`.
pub type SharedSage = Arc<Mutex<Sage>>;

/// Resolves a password for the active wallet, prompting the `main` webview.
///
/// Reads the wallet fingerprint and its stored `password_protected` flag, then
/// releases the app lock *before* the frontend round-trip. Uses the cheap
/// config flag rather than `Keychain::is_password_protected`, which runs an
/// Argon2 decrypt probe on every call.
pub async fn resolve(
    app_handle: &AppHandle,
    state: &SharedSage,
    gate: &PasswordGateState,
) -> Result<Option<String>> {
    let (fingerprint, requires_password) = {
        let sage = state.lock().await;
        let fingerprint = sage
            .wallet()
            .map_err(|err| Error { kind: err.kind(), reason: err.to_string() })?
            .fingerprint;
        let requires_password = sage
            .wallet_config
            .wallets
            .iter()
            .find(|wallet| wallet.fingerprint == fingerprint)
            .is_some_and(|wallet| wallet.password_protected);
        (fingerprint, requires_password)
    };

    let prompter = prompter::TauriPrompter { app_handle, gate };
    let verifier = SageVerifier { state };
    resolve_with(&prompter, &verifier, fingerprint, requires_password).await
}

/// Verifies a candidate password by taking the Sage lock briefly, then
/// releasing it before the next prompt. Never holds the lock across an await.
struct SageVerifier<'a> {
    state: &'a SharedSage,
}

#[async_trait::async_trait]
impl PasswordVerifier for SageVerifier<'_> {
    async fn verify(&self, fingerprint: u32, password: &str) -> Result<bool> {
        let sage = self.state.lock().await;
        match sage.keychain.extract_secrets(fingerprint, password.as_bytes()) {
            Ok(_) => Ok(true),
            Err(sage_keychain::KeychainError::Decrypt) => Ok(false),
            Err(err) => Err(Error {
                kind: sage_api::ErrorKind::Internal,
                reason: err.to_string(),
            }),
        }
    }
}
```

`Keychain` is deliberately **not** cloned and `crates/sage-keychain/` is **not** modified. Both lock
scopes above are tight and neither spans an await, so the frontend round-trip always happens with the
Sage lock released. Add `sage-keychain = { workspace = true }` to the gate crate's manifest if Step 1
omitted it.

- [ ] **Step 6: Add the response command**

Add to `src-tauri/src/commands.rs`:

```rust
use sage_password_gate::{PasswordGateState, PasswordOutcome};

#[command]
#[specta]
pub async fn submit_password_response(
    gate: State<'_, PasswordGateState>,
    request_id: String,
    outcome: PasswordOutcome,
) -> Result<()> {
    Ok(gate.deliver(&request_id, outcome)?)
}
```

(Make it `gate.deliver(...).await` if Step 3 chose the async form.)

- [ ] **Step 7: Register state, command, and event**

In `src-tauri/src/lib.rs`:

1. Add `commands::submit_password_response,` to the `sage_commands!` list in `collect_commands![...]`.
2. Change **both** `.events(collect_events![SyncEvent])` occurrences (the `specta_builder` one near
   line 187 and the `#[cfg(mobile)]` one near line 215) to
   `.events(collect_events![SyncEvent, password_gate::PasswordRequest])`.
3. Add `.manage(password_gate::PasswordGateState::default())` to the `tauri::Builder` chain, alongside
   the other `.manage(...)` calls.
4. Add `use crate::password_gate;` if not already in scope.

- [ ] **Step 8: Build and regenerate bindings**

```bash
export SDKROOT="$(xcrun --show-sdk-path)"
cargo build -p sage-password-gate -p sage-tauri
pnpm run generate:bindings
git diff --stat src/bindings.ts
```

Expected: builds clean; `src/bindings.ts` gains `submitPasswordResponse`, `PasswordRequest`,
`PasswordOutcome`, `PasswordAttemptError`, and a `passwordRequest` entry in `events`.

- [ ] **Step 9: Commit (only with user approval)**

```bash
git add crates/sage-password-gate crates/sage-keychain/src src-tauri/src src/bindings.ts
git commit -m "feat(password-gate): wire main-window transport for password requests"
```

---

## Task 3: Macro support and the drift-proof gated endpoint set

**Files:**

- Create: `crates/sage-api/password-gated.json`
- Modify: `crates/sage-api/macro/src/lib.rs`
- Modify: `crates/sage-api/src/lib.rs` (drift test)

**Interfaces:**

- Produces: a `maybe_unlock` token usable inside `impl_endpoints_tauri!`'s `repeat` block, expanding to
  the gate call for gated endpoints and to nothing otherwise.

- [ ] **Step 1: Create the gated endpoint set**

Create `crates/sage-api/password-gated.json`. These 32 names are exactly the endpoints whose request
type carries a `password: Option<String>` field:

```json
[
  "add_nft_uri",
  "assign_nfts_to_did",
  "auto_combine_cat",
  "auto_combine_xch",
  "bulk_mint_nfts",
  "bulk_send_cat",
  "bulk_send_xch",
  "cancel_offer",
  "cancel_offers",
  "combine",
  "create_did",
  "create_transaction",
  "delete_key",
  "exercise_options",
  "finalize_clawback",
  "get_secret_key",
  "increase_derivation_index",
  "issue_cat",
  "make_offer",
  "mint_option",
  "multi_send",
  "normalize_dids",
  "send_cat",
  "send_xch",
  "sign_coin_spends",
  "sign_message_by_address",
  "sign_message_with_public_key",
  "split",
  "take_offer",
  "transfer_dids",
  "transfer_nfts",
  "transfer_options"
]
```

- [ ] **Step 2: Write the failing drift test**

Add to `crates/sage-api/src/lib.rs`:

```rust
#[cfg(test)]
mod password_gate_drift {
    use std::collections::BTreeSet;

    /// The gated set must match, exactly, the request types carrying a
    /// `password` field. If this fails you either added a signing endpoint
    /// without gating it (a security hole) or gated one that takes no
    /// password (a spurious prompt).
    #[test]
    fn gated_set_matches_request_types_with_password_field() {
        let gated: BTreeSet<String> =
            serde_json::from_str(include_str!("../password-gated.json")).unwrap();

        let mut discovered = BTreeSet::new();
        for source in [
            include_str!("requests/action_system.rs"),
            include_str!("requests/actions.rs"),
            include_str!("requests/keys.rs"),
            include_str!("requests/offers.rs"),
            include_str!("requests/transactions.rs"),
            include_str!("requests/wallet_connect.rs"),
        ] {
            discovered.extend(structs_with_password_field(source));
        }

        assert_eq!(
            gated, discovered,
            "password-gated.json is out of sync with the request types.\n\
             Only in password-gated.json: {:?}\n\
             Only in request types: {:?}",
            gated.difference(&discovered).collect::<Vec<_>>(),
            discovered.difference(&gated).collect::<Vec<_>>(),
        );
    }

    /// Scans Rust source for `pub struct Name {` blocks containing a
    /// `pub password: Option<String>` field, returning snake_case names.
    fn structs_with_password_field(source: &str) -> BTreeSet<String> {
        let mut found = BTreeSet::new();
        let lines: Vec<&str> = source.lines().collect();
        let mut index = 0;

        while index < lines.len() {
            let line = lines[index].trim_end();
            let Some(rest) = line.strip_prefix("pub struct ") else {
                index += 1;
                continue;
            };
            let Some(name) = rest.strip_suffix(" {") else {
                index += 1;
                continue;
            };

            let mut cursor = index + 1;
            let mut has_password = false;
            while cursor < lines.len() && lines[cursor] != "}" {
                if lines[cursor].trim() == "pub password: Option<String>," {
                    has_password = true;
                }
                cursor += 1;
            }

            if has_password {
                found.insert(to_snake_case(name));
            }
            index = cursor + 1;
        }

        found
    }

    fn to_snake_case(name: &str) -> String {
        let mut out = String::new();
        for (position, character) in name.char_indices() {
            if character.is_uppercase() && position != 0 {
                out.push('_');
            }
            out.extend(character.to_lowercase());
        }
        out
    }
}
```

Note the empty-struct guard: `pub struct DeleteDatabaseResponse {}` is written on one line and so is
correctly skipped by the `" {"` suffix check. Do not loosen that check.

- [ ] **Step 3: Run the test to verify it fails**

```bash
export SDKROOT="$(xcrun --show-sdk-path)"
cargo test -p sage-api password_gate_drift
```

Expected: FAIL — `couldn't read ../password-gated.json` if Step 1 was skipped, otherwise it should
already PASS. If it fails with a set mismatch, **the JSON in Step 1 is authoritative only if the code
agrees** — re-derive the list from the actual source and fix the JSON, do not weaken the test.

- [ ] **Step 4: Add `maybe_unlock` to the macro**

In `crates/sage-api/macro/src/lib.rs`, inside `generate()`, load the gated set next to the endpoints:

```rust
let password_gated: std::collections::BTreeSet<String> =
    serde_json::from_str(include_str!("../../password-gated.json"))
        .expect("Invalid password-gated endpoint file");
```

Thread `&password_gated` through `convert()` as an extra parameter (alongside `endpoints`), then add a
branch in the `TokenTree::Ident` arm, next to the existing `maybe_async` / `maybe_await` branches:

```rust
} else if ident == "maybe_unlock" {
    if password_gated.contains(endpoint) {
        output.extend(quote!(
            req.password = sage_password_gate::resolve(&app_handle, state.inner(), gate.inner()).await?;
        ));
    }
}
```

The `maybe_unlock` identifier is snake_case, so this branch **must** appear before the
`ident.is_case(Case::Snake)` branch, exactly as `maybe_async` and `maybe_await` do. Otherwise it will
be rewritten into an endpoint name instead of expanded.

- [ ] **Step 5: Verify the macro compiles**

```bash
export SDKROOT="$(xcrun --show-sdk-path)"
cargo build -p sage-api-macro
cargo test -p sage-api password_gate_drift
```

Expected: both PASS.

- [ ] **Step 6: Commit (only with user approval)**

```bash
git add crates/sage-api/password-gated.json crates/sage-api/macro/src/lib.rs crates/sage-api/src/lib.rs
git commit -m "feat(password-gate): add maybe_unlock macro token and drift test"
```

---

## Task 4: Gate every endpoint command

**Files:**

- Modify: `src-tauri/src/commands.rs:67-75`

**Interfaces:**

- Consumes: `password_gate::resolve` (Task 2), `maybe_unlock` (Task 3)
- Produces: all 32 gated Tauri commands now resolve their own password

- [ ] **Step 1: Update the repeat block**

Replace the `impl_endpoints_tauri!` block at `src-tauri/src/commands.rs:67-75` with:

```rust
impl_endpoints_tauri! {
    (repeat
        #[command]
        #[specta]
        pub async fn endpoint(
            app_handle: AppHandle,
            state: State<'_, AppState>,
            gate: State<'_, PasswordGateState>,
            mut req: Endpoint,
        ) -> Result<EndpointResponse> {
            maybe_unlock;
            Ok(state.lock().await.endpoint(req) maybe_await?)
        }
    )
}
```

Two consequences to expect from the compiler:

- Ungated endpoints now take `app_handle`, `gate`, and `mut req` without using them. Silence this by
  prefixing with underscores is **not** possible inside the repeat block, so instead add
  `#[allow(unused_variables, unused_mut)]` immediately above `pub async fn endpoint`.
- Adding parameters changes the generated command signatures. Tauri injects `AppHandle` and `State`
  automatically, so the **TypeScript call signatures are unchanged** — `req` remains the only argument.
  Confirm this in the bindings diff in Step 3.

- [ ] **Step 2: Build**

```bash
export SDKROOT="$(xcrun --show-sdk-path)"
cargo build -p sage-password-gate -p sage-tauri
```

Expected: clean build. If a specific endpoint fails because its request type has no `password` field
but appears in `password-gated.json`, the drift test in Task 3 was wrong — fix the JSON, not this block.

- [ ] **Step 3: Regenerate bindings and confirm TS signatures did not change**

```bash
pnpm run generate:bindings
git diff src/bindings.ts | grep -E '^\-.*sendXch|^\+.*sendXch'
```

Expected: no change to `sendXch`'s TypeScript signature. If the signature gained parameters, the
`AppHandle`/`State` injection is not being recognized — stop and report.

- [ ] **Step 4: Confirm the RPC path is untouched**

```bash
export SDKROOT="$(xcrun --show-sdk-path)"
cargo test -p sage-rpc
```

Expected: PASS, unchanged. This is the regression canary for the headless path — if these fail, the
gate has leaked into the core.

- [ ] **Step 5: Commit (only with user approval)**

```bash
git add src-tauri/src/commands.rs src/bindings.ts
git commit -m "feat(password-gate): resolve passwords in all gated endpoint commands"
```

---

## Task 5: Bridge — gate app requests after approval

**Files:**

- Modify: `crates/sage-apps/src/bridge/bridge_request.rs:83-135` (`process_after_approval`)
- Modify: `crates/sage-apps/src/bridge/types.rs` (`BridgeTools`)
- Modify: `crates/sage-apps/src/bridge/methods/user/wallet/send_xch.rs`
- Modify: `crates/sage-apps/src/bridge/methods/user/wallet/sign_message.rs`
- Modify: `crates/sage-apps/src/bridge/methods/user/wallet/sign_coin_spends.rs`
- Modify: `crates/sage-apps/src/runtime/manager.rs` (expose `hide_runtime_inner` to the bridge)

**Interfaces:**

- Consumes: `password_gate::resolve` (Task 2)
- Produces: `BridgeTools` gains `pub password: Option<String>`, defaulted to `None` on every existing
  construction site.

- [ ] **Step 1: Add the field to `BridgeTools`**

In `crates/sage-apps/src/bridge/types.rs`, add to the `BridgeTools` struct:

```rust
/// Password resolved by the main-window gate, for methods that sign.
/// `None` when the wallet is unprotected or the method does not sign.
pub password: Option<String>,
```

Then fix every construction of `BridgeTools` in `bridge_request.rs` (there are three: in
`process_shared`'s `prepare_approval` call, in `process_shared`'s non-approval `execute_bridge_request`
call, and in `execute_bridge_request` itself) to pass `password: None` for now. The build must be green
before moving on.

- [ ] **Step 2: Thread the password through `execute_bridge_request`**

Change `execute_bridge_request` in `bridge_request.rs` to take an extra parameter and use it:

```rust
async fn execute_bridge_request(
    app_handle: &AppHandle,
    app_state: &State<'_, AppState>,
    origin: &BridgeOrigin,
    registry: BridgeRegistry,
    request: &RustBridgeRequest,
    password: Option<String>,
) -> RustBridgeResponse {
```

and inside, set `password` on the `BridgeTools` it constructs. Update both call sites in
`process_shared` to pass `None`, and add the same parameter to `process_shared` so
`process_after_approval` can supply it.

- [ ] **Step 3: Call the gate in `process_after_approval`**

In `process_after_approval`, the final `else` branch currently calls `process_shared(...)`. Replace that
branch with:

```rust
} else {
    // Hide the approval app before prompting: app runtimes are sibling
    // webviews inside the same window and would cover the main webview's
    // password dialog.
    hide_bridge_approval_runtime(app_handle, apps_state).await;

    match password_gate_resolve(app_handle, app_state).await {
        Ok(password) => {
            process_shared(
                app_handle,
                app_state,
                &origin,
                pending.registry_kind,
                &pending.request,
                true,
                password,
            )
            .await?
        }
        Err(err) => RustBridgeInvokeResult::error(
            &pending.request.id,
            "unauthorized",
            err.to_string(),
        ),
    }
}
```

The existing `expires_at_ms` check stays exactly where it is, _above_ this branch. Because the check
already ran before the prompt, add a second check immediately after the gate returns, so a prompt that
outlives the deadline fails correctly:

```rust
if unix_timestamp_ms() as u64 > pending.expires_at_ms {
    RustBridgeInvokeResult::error(
        &pending.request.id,
        "approval_timeout",
        "Approval expired during password entry".to_string(),
    )
}
```

Place this as a guard inside the `Ok(password)` arm, before `process_shared`.

- [ ] **Step 4: Add the two helper functions**

`hide_bridge_approval_runtime` finds the runtime whose app id is `SYSTEM_APP_BRIDGE_APPROVAL_ID`
(`crates/sage-apps/src/system_apps.rs:22`) and calls the existing `hide_runtime_inner`
(`crates/sage-apps/src/runtime/manager.rs:395`), then emits the resulting `RuntimeChangeSet`. Make
`hide_runtime_inner` and `RuntimeChangeSet` visible to the bridge module by widening their visibility
from private to `pub(crate)`. Failure to hide is logged with `tracing::warn!` and does not abort the
request — a covered dialog is a UX problem, not a security one.

`password_gate_resolve` is a thin call into `sage-password-gate`, which `sage-apps` depends on
directly — this is exactly why the gate was made its own crate in Task 1 rather than living in
`src-tauri/src/`:

```rust
async fn password_gate_resolve(
    app_handle: &AppHandle,
    app_state: &State<'_, AppState>,
) -> Result<Option<String>, String> {
    let gate = app_handle.state::<sage_password_gate::PasswordGateState>();
    sage_password_gate::resolve(app_handle, app_state.inner(), &gate)
        .await
        .map_err(|err| err.reason)
}
```

Add `sage-password-gate = { workspace = true }` to `crates/sage-apps/Cargo.toml`. `AppState` in
`sage-apps` is already `Arc<Mutex<Sage>>`, matching `sage_password_gate::SharedSage`; if the types do
not unify, pass `app_state.inner().clone()` and take a reference to that.

- [ ] **Step 5: Use the password in the three wallet methods**

In each of `send_xch.rs`, `sign_message.rs`, and `sign_coin_spends.rs`, the `From<Params>` impl
currently hardcodes `password: None`. Remove the `password` field from those `From` impls and set it in
`handle` instead. For `send_xch.rs`:

```rust
async fn handle(
    &self,
    _ctx: BridgeContext<'_>,
    tools: BridgeTools<'_>,
    request: &RustBridgeRequest,
) -> BridgeHandleResult {
    let params: WalletSendXchParams = parse_required_params(self, request)?;
    let mut req: SendXch = params.into();
    req.password = tools.password.clone();

    let result = tools
        .app_state
        .lock()
        .await
        .send_xch(req)
        .await
        .map_err(|err| {
            BridgeMethodHandleError::internal_error(format!("{} failed: {err}", self.name()))
        })?;

    Ok(Box::new(result))
}
```

Apply the same shape to `sign_message.rs` (`SignMessageWithPublicKey`) and `sign_coin_spends.rs`
(`SignCoinSpends`). Keep `password: None` in the `From` impls only if removing it breaks struct
initialization — in that case leave `password: None` there and let the `handle` assignment override it.

- [ ] **Step 6: Build and test**

```bash
export SDKROOT="$(xcrun --show-sdk-path)"
cargo build --workspace
cargo test -p sage-apps
```

Expected: clean build, existing `sage-apps` tests pass.

- [ ] **Step 7: Commit (only with user approval)**

```bash
git add crates/sage-apps crates/sage-password-gate src-tauri
git commit -m "feat(password-gate): prompt in main window for app bridge requests"
```

---

## Task 6: Force approval on protected wallets despite auto-submit

**Files:**

- Modify: `crates/sage-apps/src/bridge/methods/user/wallet/send_xch.rs` (`approval_request`)
- Test: `crates/sage-apps/src/bridge/methods/user/wallet/send_xch.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**

- Consumes: `BridgeContext` (existing), the wallet `password_protected` flag
- Produces: no new public interface

- [ ] **Step 1: Write the failing test**

Add to `send_xch.rs`:

```rust
#[cfg(test)]
mod tests {
    /// A password-protected wallet must always produce an approval, even when
    /// the app holds WalletSendXchAutoSubmit. Silent auto-submit is
    /// incompatible with password protection: there would be no UI moment in
    /// which to collect the password.
    #[test]
    fn protected_wallet_forces_approval_despite_auto_submit_grant() {
        assert!(super::requires_approval(
            /* auto_submit_granted */ true,
            /* wallet_protected */ true,
        ));
    }

    #[test]
    fn unprotected_wallet_still_honours_auto_submit_grant() {
        assert!(!super::requires_approval(true, false));
    }

    #[test]
    fn without_the_grant_approval_is_always_required() {
        assert!(super::requires_approval(false, false));
        assert!(super::requires_approval(false, true));
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

```bash
export SDKROOT="$(xcrun --show-sdk-path)"
cargo test -p sage-apps send_xch
```

Expected: FAIL to compile — `cannot find function 'requires_approval'`.

- [ ] **Step 3: Extract the predicate and use it**

Add to `send_xch.rs`:

```rust
/// Whether this request needs a user approval step.
fn requires_approval(auto_submit_granted: bool, wallet_protected: bool) -> bool {
    !auto_submit_granted || wallet_protected
}
```

Then change `approval_request` so its early return consults the predicate. The existing body is:

```rust
if ctx
    .app
    .is_capability_granted(UserBridgeCapability::WalletSendXchAutoSubmit.into())
{
    return Ok(None);
}
```

Replace with a call to `requires_approval`, reading the active wallet's `password_protected` flag from
`ctx`. `approval_request` is synchronous and `BridgeContext` currently carries only `app`, so add the
flag to `BridgeContext` — populate it where `BridgeContext { app }` is constructed in
`bridge_request.rs` (three sites), reading it from `app_state` the same way `active_wallet_fingerprint`
does. Return early only when `requires_approval(..)` is `false`.

- [ ] **Step 4: Run the tests to verify they pass**

```bash
export SDKROOT="$(xcrun --show-sdk-path)"
cargo test -p sage-apps
```

Expected: PASS.

- [ ] **Step 5: Commit (only with user approval)**

```bash
git add crates/sage-apps/src/bridge
git commit -m "feat(password-gate): force approval for protected wallets on auto-submit"
```

---

## Task 7: Invert PasswordContext into a responder

**Files:**

- Modify: `src/contexts/PasswordContext.tsx`
- Modify: `src/hooks/usePassword.ts`
- Modify: `src/pages/Settings.tsx:1106`, `src/pages/Settings.tsx:1123`

**Interfaces:**

- Consumes: `events.passwordRequest`, `commands.submitPasswordResponse` from `src/bindings.ts`
- Produces: `PasswordContextType { requireLocalAuth: () => Promise<boolean> }`.
  `requestPassword` is **removed** — later tasks depend on it being gone.

- [ ] **Step 1: Rewrite the provider**

Replace the body of `src/contexts/PasswordContext.tsx` with:

```tsx
import { PasswordDialog } from '@/components/dialogs/PasswordDialog';
import { useBiometric } from '@/hooks/useBiometric';
import { commands, events, PasswordRequest } from '@/bindings';
import { platform } from '@tauri-apps/plugin-os';
import {
  createContext,
  ReactNode,
  useCallback,
  useEffect,
  useRef,
  useState,
} from 'react';

const isMobile = platform() === 'ios' || platform() === 'android';

// Biometric caching interval (5 minutes)
const BIOMETRIC_CACHE_MS = 5 * 60 * 1000;

export interface PasswordContextType {
  /**
   * UI-only authentication gate for actions that touch no wallet secret
   * (starting the RPC server, toggling run-on-startup). Returns true if the
   * caller may proceed. This deliberately does NOT go through the Rust
   * password gate — there is no unlock operation behind it.
   */
  requireLocalAuth: () => Promise<boolean>;
}

export const PasswordContext = createContext<PasswordContextType | undefined>(
  undefined,
);

export function PasswordProvider({ children }: { children: ReactNode }) {
  const [pending, setPending] = useState<PasswordRequest | null>(null);
  const { enabled: biometricEnabled } = useBiometric();
  const lastBiometricPromptRef = useRef<number | null>(null);

  const runBiometric = useCallback(async (): Promise<boolean> => {
    const now = performance.now();
    if (
      lastBiometricPromptRef.current !== null &&
      now - lastBiometricPromptRef.current < BIOMETRIC_CACHE_MS
    ) {
      return true;
    }
    try {
      const { authenticate } = await import('@tauri-apps/plugin-biometric');
      await authenticate('Authenticate to continue', {
        allowDeviceCredential: false,
      });
      lastBiometricPromptRef.current = now;
      return true;
    } catch {
      return false;
    }
  }, []);

  const requireLocalAuth = useCallback(async (): Promise<boolean> => {
    if (!biometricEnabled || !isMobile) return true;
    return runBiometric();
  }, [biometricEnabled, runBiometric]);

  useEffect(() => {
    const unlisten = events.passwordRequest.listen(async ({ payload }) => {
      // Case 1: password takes precedence — show the dialog and wait.
      if (payload.requiresPassword) {
        setPending(payload);
        return;
      }

      // Case 2: no password, biometric enabled — standalone gate with cache.
      if (biometricEnabled && isMobile) {
        const ok = await runBiometric();
        await commands.submitPasswordResponse(
          payload.requestId,
          ok ? { kind: 'no_auth_needed' } : { kind: 'cancelled' },
        );
        return;
      }

      // Case 3: no password, no biometric — nothing to do.
      await commands.submitPasswordResponse(payload.requestId, {
        kind: 'no_auth_needed',
      });
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [biometricEnabled, runBiometric]);

  const handleSubmit = useCallback(
    (password: string) => {
      if (!pending) return;
      setPending(null);
      commands.submitPasswordResponse(pending.requestId, {
        kind: 'password',
        password,
      });
    },
    [pending],
  );

  const handleCancel = useCallback(() => {
    if (!pending) return;
    setPending(null);
    commands.submitPasswordResponse(pending.requestId, { kind: 'cancelled' });
  }, [pending]);

  return (
    <PasswordContext.Provider value={{ requireLocalAuth }}>
      {children}
      <PasswordDialog
        open={pending !== null}
        onSubmit={handleSubmit}
        onCancel={handleCancel}
      />
    </PasswordContext.Provider>
  );
}
```

Verify the generated `PasswordOutcome` discriminant names against `src/bindings.ts` — the Rust type uses
`#[serde(tag = "kind", rename_all = "snake_case")]`, so `no_auth_needed`, `cancelled`, and `password`
are expected. If the generated shape differs, match the bindings, not this snippet.

`src/hooks/usePassword.ts` needs no change — it re-exports whatever the context provides.

- [ ] **Step 2: Show the retry error in the dialog**

`PasswordDialog` currently takes no error prop. Add an optional one so a wrong password re-prompts
visibly rather than silently reopening:

In `src/components/dialogs/PasswordDialog.tsx`, extend `PasswordDialogProps` with
`attemptsRemaining?: number`, and render, below the existing `DialogDescription`:

```tsx
{
  attemptsRemaining !== undefined && (
    <p className='text-sm text-destructive'>
      <Trans>Incorrect password. {attemptsRemaining} attempts remaining.</Trans>
    </p>
  );
}
```

Pass it from the provider: `attemptsRemaining={pending?.error?.attemptsRemaining}`.

Do **not** run `pnpm run extract` for the new string — see Global Constraints.

- [ ] **Step 3: Convert the Settings call sites**

In `src/pages/Settings.tsx`, replace both `requestPassword(false)` gates. At line ~1106:

```tsx
const start = async () => {
  if (!(await requireLocalAuth())) return;

  commands
    .startRpcServer()
    .catch(addError)
    .then(() => setIsRunning(true));
};
```

At line ~1123:

```tsx
const toggleRunOnStartup = async (checked: boolean) => {
  if (!(await requireLocalAuth())) return;

  commands
    .setRpcRunOnStartup(checked)
    .catch(addError)
    .then(() => setRunOnStartup(checked));
};
```

Change the destructure at line ~1083 from `const { requestPassword } = usePassword();` to
`const { requireLocalAuth } = usePassword();`.

- [ ] **Step 4: Verify**

```bash
pnpm run build:frontend
pnpm run lint
```

Expected: `tsc -b` fails **only** in the files Tasks 8 and 9 will fix (`WalletCard.tsx`,
`ConfirmationDialog.tsx`, `useOfferProcessor.ts`, `Offer.tsx`, `WalletConnectContext.tsx`,
`src/walletconnect/*`), because `requestPassword` no longer exists. `Settings.tsx` and
`PasswordContext.tsx` must be clean. Record the remaining error list — it is the worklist for the next
two tasks.

- [ ] **Step 5: Commit (only with user approval)**

```bash
git add src/contexts/PasswordContext.tsx src/components/dialogs/PasswordDialog.tsx src/pages/Settings.tsx
git commit -m "feat(password-gate): invert PasswordContext into a responder"
```

---

## Task 8: Remove password plumbing from React call sites

**Files:**

- Modify: `src/components/WalletCard.tsx:69,82,180,196`
- Modify: `src/components/ConfirmationDialog.tsx:70,534,663`
- Modify: `src/hooks/useOfferProcessor.ts:31,65,188`
- Modify: `src/pages/Offer.tsx:37,111`
- Modify: `src/pages/Settings.tsx` — the `WalletSettings` component's `usePassword()` at ~:1167 and its `requestPassword` call gating `increaseDerivationIndex`

**Interfaces:**

- Consumes: `PasswordContextType` from Task 7 (no `requestPassword`)
- Produces: no component's public props change

- [ ] **Step 1: Apply the mechanical edit in each file**

Note on `Settings.tsx`: Task 7 converted two call sites in that file to `requireLocalAuth()` because they gate UI-only actions (starting the RPC server, run-on-startup) with no wallet secret behind them. The `WalletSettings` site is **different** and must be treated as an ordinary Task 8 site: `increase_derivation_index` is one of the 32 password-gated endpoints, so Rust now resolves its password itself. Strip the plumbing — do NOT convert it to `requireLocalAuth()`. After this task `usePassword()` should remain in the file only for the two `requireLocalAuth` consumers.

In every listed file the pattern is identical. Remove:

1. The `const { requestPassword } = usePassword();` destructure (and the whole `usePassword` import if
   nothing else in the file uses it).
2. The `const password = await requestPassword(...); if (password === undefined) return;` guard.
3. The `password` property from the command argument object.
4. `requestPassword` from any `useCallback` / `useMemo` dependency array.

Concretely, `src/pages/Offer.tsx:111` currently reads:

```tsx
const password = await requestPassword(wallet?.has_password ?? false);
if (password === undefined) return;
```

Delete both lines, and drop `password` from the `commands.takeOffer({ ... })` argument below them.

Keep the surrounding `try`/`catch` and `addError` handling exactly as-is. A cancelled prompt now
surfaces as a rejected command with the `Unauthorized` kind and the reason
`"Password entry cancelled"` — Task 10 makes the error handler silent for that case.

- [ ] **Step 2: Verify**

```bash
pnpm run build:frontend
pnpm run lint
```

Expected: the four files in this task no longer appear in the `tsc` error list. Only
`WalletConnectContext.tsx` and `src/walletconnect/*` remain.

- [ ] **Step 3: Commit (only with user approval)**

```bash
git add src/components/WalletCard.tsx src/components/ConfirmationDialog.tsx src/hooks/useOfferProcessor.ts src/pages/Offer.tsx
git commit -m "refactor(password-gate): drop password plumbing from React call sites"
```

---

## Task 9: Remove password plumbing from the WalletConnect layer

**Files:**

- Modify: `src/walletconnect/handler.ts:28`
- Modify: `src/walletconnect/commands/chip0002.ts:79,106`
- Modify: `src/walletconnect/commands/high-level.ts:50,87,97`
- Modify: `src/walletconnect/commands/offers.ts:9,38,55`
- Modify: `src/contexts/WalletConnectContext.tsx:69,109,146`

**Interfaces:**

- Consumes: `PasswordContextType` from Task 7
- Produces: the handler context type loses both `requestPassword` and `hasPassword`

- [ ] **Step 1: Narrow the handler context type**

In `src/walletconnect/handler.ts`, delete line 28:

```ts
requestPassword: (hasPassword: boolean) => Promise<string | null | undefined>;
```

and delete the `hasPassword` field from the same interface if present. Nothing replaces them — the
Rust gate now owns this.

- [ ] **Step 2: Strip the prompts from the command modules**

In `chip0002.ts`, `high-level.ts`, and `offers.ts`, at each listed line, delete the pair:

```ts
const password = await context.requestPassword(context.hasPassword);
if (password === undefined) throw new Error('Authentication failed');
```

and remove `password` from the command argument object immediately below. In `chip0002.ts:109` the
call is `commands.signMessageWithPublicKey({ ...params, password })` — it becomes
`commands.signMessageWithPublicKey({ ...params })`.

- [ ] **Step 3: Stop supplying the removed fields**

In `src/contexts/WalletConnectContext.tsx`:

- Delete `const { requestPassword } = usePassword();` (line ~69) and the `usePassword` import if now
  unused.
- Delete `requestPassword,` from the context object passed to the handler (line ~109).
- Remove `requestPassword` and `wallet?.has_password` from the `useMemo`/`useCallback` dependency array
  (line ~146). Leave `signClient`, `addError`, and `isReadOnly` in place.

- [ ] **Step 4: Verify**

```bash
pnpm run build:frontend
pnpm run lint
```

Expected: `tsc -b` now PASSES with zero errors, and `eslint` reports no new warnings. If
`wallet?.has_password` is now unused in this file, remove the `wallet` destructure too.

**`tsc` alone does not prove this task is complete.** The four `src/walletconnect/` files type-check
against their own local `HandlerContext` interface rather than against `PasswordContextType`, so they
did not appear as compiler errors even while still calling `requestPassword`. A green build is
therefore necessary but not sufficient. Confirm the removal directly:

```bash
grep -rn "requestPassword\|hasPassword" src/walletconnect/ src/contexts/WalletConnectContext.tsx
```

Expected: no matches. Any hit is an unremoved call site regardless of what `tsc` reports.

- [ ] **Step 5: Commit (only with user approval)**

```bash
git add src/walletconnect src/contexts/WalletConnectContext.tsx
git commit -m "refactor(password-gate): drop password plumbing from WalletConnect layer"
```

---

## Task 10: Silence cancelled-prompt errors

A user dismissing the password dialog must not produce an error toast. Before this task, cancelling any
operation surfaces `"Password entry cancelled"` as a failure.

**Files:**

- Modify: `src/contexts/ErrorContext.tsx`

**Interfaces:**

- Consumes: `CANCELLED_REASON` value `"Password entry cancelled"` from Task 1, and `ErrorKind`
  `unauthorized`
- Produces: no new public interface

- [ ] **Step 1: Read the current error handling**

```bash
grep -n "IncorrectPassword\|incorrect_password\|addError" src/contexts/ErrorContext.tsx | head -20
```

Note how `addError` decides to surface an error, and whether an existing branch already special-cases
password errors.

- [ ] **Step 2: Add the silent branch**

In `src/contexts/ErrorContext.tsx`, inside `addError`, return early without displaying anything when
the error is a deliberate cancellation:

```ts
const PASSWORD_CANCELLED_REASON = 'Password entry cancelled';

// ... inside addError, before any state update:
if (
  error.kind === 'unauthorized' &&
  error.reason === PASSWORD_CANCELLED_REASON
) {
  return;
}
```

Match the exact string to `CANCELLED_REASON` in
`crates/sage-password-gate/src/resolve.rs`. If they
drift the toast reappears, so keep the comment noting the coupling.

- [ ] **Step 3: Verify**

```bash
pnpm run build:frontend
pnpm run lint
```

Expected: PASS.

- [ ] **Step 4: Commit (only with user approval)**

```bash
git add src/contexts/ErrorContext.tsx
git commit -m "fix(password-gate): stay silent when the user cancels the prompt"
```

---

## Task 11: Full verification and manual smoke

No new code. This task proves the feature works end to end and that nothing regressed.

**Files:** none modified.

- [ ] **Step 1: Full workspace build and test**

```bash
export SDKROOT="$(xcrun --show-sdk-path)"
cargo build --workspace
cargo test --workspace
```

Expected: all PASS. Pay particular attention to `sage-rpc` — those tests exercise the headless
password path and must be unchanged. Any failure there means the gate leaked into the core.

- [ ] **Step 1b: Clear the one pre-existing clippy denial in `sage-rpc`**

`crates/sage-rpc/src/tests.rs:353` has `old_password: "".to_string()`, which `clippy::manual_string_new`
denies. It predates this branch — verified identical on the `password` base branch — but Step 2 gates on
`-D warnings`, so it must go or the gate fails on code this plan did not write.

This is the one authorised exception to the "do not modify `crates/sage-rpc/**`" constraint. That
constraint exists to stop the password gate leaking into the headless core; a `String::new()` lint fix in
a test file is not that. Change it to `old_password: String::new(),` and nothing else. Commit it
separately, labelled as a pre-existing lint fix, so it stays trivially separable from the feature.

```bash
git add crates/sage-rpc/src/tests.rs
git commit -m "chore: fix pre-existing manual_string_new lint in sage-rpc tests"
```

- [ ] **Step 2: Lint and format**

```bash
export SDKROOT="$(xcrun --show-sdk-path)"
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
pnpm run lint
pnpm run prettier:check
```

Expected: clean. Run `cargo fmt --all` and `pnpm run prettier` to fix formatting if needed.

- [ ] **Step 3: Confirm no forbidden files changed**

```bash
git diff --name-only password...HEAD | grep -E '^crates/sage/|^crates/sage-rpc/' || echo "clean: core untouched"
```

Expected: `clean: core untouched`. If anything is listed, review it against the Global Constraints
before proceeding.

- [ ] **Step 4: Confirm no .po churn**

```bash
git diff --name-only password...HEAD | grep '\.po$' || echo "clean: no lingui churn"
```

Expected: `clean: no lingui churn`.

- [ ] **Step 5: Manual smoke — the three responder branches**

```bash
export SDKROOT="$(xcrun --show-sdk-path)"
pnpm run tauri:dev
```

Walk through each and confirm:

1. **Protected wallet, main UI.** Send a small amount of XCH. The password dialog appears. Enter the
   wrong password twice — it re-prompts each time showing the remaining attempt count. Enter the
   correct password — the transaction proceeds.
2. **Protected wallet, three strikes.** Repeat with three wrong passwords. The operation fails with an
   authorization error, and the dialog closes.
3. **Cancel.** Start a send and dismiss the dialog. No error toast appears, and nothing is submitted.
4. **Unprotected wallet.** Send XCH. No dialog appears at all.
5. **Unprotected wallet with biometrics (mobile only).** The biometric gate appears, and a second
   operation within five minutes does not re-prompt.

- [ ] **Step 6: Manual smoke — the app bridge**

With a **password-protected** wallet active, launch an app that calls `wallet.sendXch` and confirm:

1. The `bridge-approval` app shows the transaction summary.
2. Approving it hides that app and reveals the password dialog **in the main window** — not inside the
   app's webview.
3. The correct password completes the request; the app receives a success response.
4. Cancelling the password dialog fails the request and the app receives an error.
5. Grant the app `WalletSendXchAutoSubmit` and repeat: an approval **still** appears, because the
   wallet is protected.
6. Switch to an **unprotected** wallet, keep the auto-submit grant, and repeat: no approval appears.

- [ ] **Step 7: Manual smoke — WalletConnect**

Pair a WalletConnect dApp against a protected wallet and confirm a signing request prompts for the
password in the main window and completes.

- [ ] **Step 8: Commit any formatting fixes (only with user approval)**

```bash
git add -A
git commit -m "chore(password-gate): formatting and lint fixes"
```
