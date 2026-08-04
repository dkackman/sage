# Passkey Unlock Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a user unlock a password-protected sage key with a passkey (Face ID / Touch ID / security key), with the typed password always available as fallback.

**Architecture:** Integrate the local `tauri-plugin-passkey` into sage. A passkey's PRF/hmac-secret output (32 bytes) is used directly as an AES-256-GCM key to wrap the key's existing password; the wrapped blob plus public enrollment metadata is stored per-wallet in `sage-config`. On unlock, a passkey assertion yields the PRF secret, which unwraps the password, which flows into the unchanged keychain decrypt path. `sage-keychain` is untouched.

**Tech Stack:** Rust (Tauri, `aes-gcm`, `base64`), React/TypeScript frontend, `tauri-plugin-passkey` (local path/file dep), macOS WebAuthn via associated-domains + `webauthn.dkackman.com`.

## Global Constraints

- **Chosen crypto model:** "passkey wraps the existing password." Passkey is convenience only, never load-bearing. Typed password always works. Enrollment REQUIRES the key already have a password.
- **Storage:** enrollment record lives on `sage_config::Wallet`, NOT in `sage-keychain`. `sage-keychain` crypto and serialization must not change.
- **PRF secret transport:** the raw 32-byte PRF secret and salts cross the Tauri IPC boundary as base64. Backend uses **standard** base64 (`base64::engine::general_purpose::STANDARD`) for `prf_secret` and `wrapped_password`. `credential_id` and `prf_salt` are stored opaquely as the base64url strings WebAuthn produces.
- **Relying party:** `RP_ID = "webauthn.dkackman.com"`, `RP_ORIGIN = "https://webauthn.dkackman.com"`. AASA is served by a Cloudflare Worker under our control.
- **Bundle identity (macOS):** bundle id `com.rigidnetwork.sage`, Apple Team ID `86TDY6D9V2`.
- **Build env:** every `cargo`/`tauri` build in this repo must first `export SDKROOT="$(xcrun --show-sdk-path)"` or it fails with `stdlib.h not found`.
- **Endpoint wiring pattern:** to add a backend command — (1) add request/response structs in `crates/sage-api/src/requests/keys.rs` with the 3-attribute derive stack, (2) add `"name": <is_async_bool>` to `crates/sage-api/endpoints.json`, (3) implement `pub fn name(&mut self, req) -> Result<Resp>` in `crates/sage/src/endpoints/keys.rs`, (4) add `commands::name,` to `collect_commands![]` in `src-tauri/src/lib.rs`. The Tauri wrapper, RPC route, and `src/bindings.ts` regenerate automatically (bindings on any `debug` `cargo tauri dev`/build).
- **Commits:** per project convention the user runs all git commits manually. Treat every "Commit" step as a checkpoint: stage the listed files and stop for the user to commit (do not auto-commit unless the user's chosen execution mode does so on their behalf).

---

### Task 1: Wire the plugin (build-green checkpoint)

**Files:**

- Modify: `src-tauri/Cargo.toml` (`[dependencies]`)
- Modify: `package.json` (`dependencies`)
- Modify: `src-tauri/src/lib.rs:158-161` (base `tauri_builder`)
- Modify: `src-tauri/capabilities/default.json`

**Interfaces:**

- Produces: the `tauri_plugin_passkey::init()` plugin registered on all platforms and the `plugin:passkey|*` commands available to the frontend via `tauri-plugin-passkey-api`.

- [ ] **Step 1: Build the plugin's JS bindings** (its `dist-js/` must exist before sage's frontend can resolve the `file:` dep)

Run:

```bash
cd /Users/don/src/dkackman/tauri-plugin-passkey && pnpm install && pnpm build
```

Expected: `tauri-plugin-passkey/tauri-plugin-passkey/dist-js/index.js` (and `.cjs`, `.d.ts`) exist.

- [ ] **Step 2: Add the Rust path dependency**

In `src-tauri/Cargo.toml`, under `[dependencies]` (near the other `tauri-plugin-*` entries), add:

```toml
tauri-plugin-passkey = { path = "../../tauri-plugin-passkey/tauri-plugin-passkey" }
```

(Path dependencies need not be workspace members; this keeps the plugin's own edition/lints.)

- [ ] **Step 3: Add the JS dependency**

In `package.json` `dependencies`, add:

```json
"tauri-plugin-passkey-api": "file:../tauri-plugin-passkey/tauri-plugin-passkey",
```

Then:

```bash
cd /Users/don/src/dkackman/sage && pnpm install
```

- [ ] **Step 4: Register the plugin on all platforms**

In `src-tauri/src/lib.rs`, add the passkey plugin to the BASE builder (it is cross-platform, unlike the mobile-only block). Change lines 158-161:

```rust
    let mut tauri_builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_passkey::init());
```

- [ ] **Step 5: Grant the capability**

In `src-tauri/capabilities/default.json`, add `"passkey:default"` to the `permissions` array (it applies to the `main` window on all platforms).

- [ ] **Step 6: Verify the backend compiles**

Run:

```bash
cd /Users/don/src/dkackman/sage && export SDKROOT="$(xcrun --show-sdk-path)" && cargo check -p sage-tauri
```

Expected: compiles (the plugin links).

- [ ] **Step 7: Verify the frontend resolves the plugin package**

Create a throwaway import to prove resolution, then remove it. Run:

```bash
cd /Users/don/src/dkackman/sage && node -e "require.resolve('tauri-plugin-passkey-api')" && echo OK
```

Expected: prints `OK`.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/Cargo.toml package.json pnpm-lock.yaml src-tauri/src/lib.rs src-tauri/capabilities/default.json
git commit -m "feat(passkey): wire tauri-plugin-passkey into sage"
```

---

### Task 2: Passkey wrap/unwrap crypto module

**Files:**

- Create: `crates/sage/src/passkey.rs`
- Modify: `crates/sage/src/lib.rs` (add `mod passkey;`)
- Modify: `crates/sage/Cargo.toml` (`[dependencies]`)

**Interfaces:**

- Produces:
  - `pub fn wrap_password(prf_secret: &[u8], password: &[u8], rng: &mut (impl rand::CryptoRng + rand::Rng)) -> Result<String, PasskeyError>` — returns standard-base64 of `nonce(12) ‖ ciphertext`.
  - `pub fn unwrap_password(prf_secret: &[u8], wrapped: &str) -> Result<Vec<u8>, PasskeyError>`
  - `pub enum PasskeyError` (thiserror).

- [ ] **Step 1: Add the `aes-gcm` dependency**

`crates/sage/Cargo.toml` already has `rand`, `rand_chacha`, and `base64`. Add under `[dependencies]`:

```toml
aes-gcm = { workspace = true }
```

- [ ] **Step 2: Write the failing tests**

Create `crates/sage/src/passkey.rs`:

```rust
use aes_gcm::{aead::Aead, AeadCore, Aes256Gcm, Key, KeyInit, Nonce};
use base64::{engine::general_purpose::STANDARD, Engine};
use rand::{CryptoRng, Rng};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PasskeyError {
    #[error("PRF secret must be 32 bytes, got {0}")]
    InvalidPrfSecretLength(usize),
    #[error("failed to encrypt password")]
    Encrypt,
    #[error("failed to decrypt password")]
    Decrypt,
    #[error("base64 decode error: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("wrapped password is truncated")]
    Truncated,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_chacha::ChaCha20Rng;
    use rand::SeedableRng;

    #[test]
    fn test_wrap_unwrap_roundtrip() {
        let mut rng = ChaCha20Rng::from_seed([3u8; 32]);
        let secret = [9u8; 32];
        let wrapped = wrap_password(&secret, b"correct horse", &mut rng).unwrap();
        let out = unwrap_password(&secret, &wrapped).unwrap();
        assert_eq!(out, b"correct horse");
    }

    #[test]
    fn test_wrong_secret_fails() {
        let mut rng = ChaCha20Rng::from_seed([3u8; 32]);
        let wrapped = wrap_password(&[9u8; 32], b"pw", &mut rng).unwrap();
        assert!(matches!(
            unwrap_password(&[8u8; 32], &wrapped),
            Err(PasskeyError::Decrypt)
        ));
    }

    #[test]
    fn test_bad_length_secret_rejected() {
        let mut rng = ChaCha20Rng::from_seed([3u8; 32]);
        assert!(matches!(
            wrap_password(&[0u8; 16], b"pw", &mut rng),
            Err(PasskeyError::InvalidPrfSecretLength(16))
        ));
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run:

```bash
cd /Users/don/src/dkackman/sage && export SDKROOT="$(xcrun --show-sdk-path)" && cargo test -p sage passkey:: 2>&1 | head -30
```

Expected: FAIL — `wrap_password`/`unwrap_password` not found.

- [ ] **Step 4: Implement the functions**

Add above the `#[cfg(test)]` block in `crates/sage/src/passkey.rs`:

```rust
pub fn wrap_password(
    prf_secret: &[u8],
    password: &[u8],
    rng: &mut (impl CryptoRng + Rng),
) -> Result<String, PasskeyError> {
    if prf_secret.len() != 32 {
        return Err(PasskeyError::InvalidPrfSecretLength(prf_secret.len()));
    }
    let key = Key::<Aes256Gcm>::from_slice(prf_secret);
    let cipher = Aes256Gcm::new(key);
    let nonce = Aes256Gcm::generate_nonce(&mut *rng);
    let ciphertext = cipher
        .encrypt(&nonce, password)
        .map_err(|_| PasskeyError::Encrypt)?;
    let mut combined = nonce.to_vec();
    combined.extend_from_slice(&ciphertext);
    Ok(STANDARD.encode(combined))
}

pub fn unwrap_password(prf_secret: &[u8], wrapped: &str) -> Result<Vec<u8>, PasskeyError> {
    if prf_secret.len() != 32 {
        return Err(PasskeyError::InvalidPrfSecretLength(prf_secret.len()));
    }
    let combined = STANDARD.decode(wrapped)?;
    if combined.len() < 12 {
        return Err(PasskeyError::Truncated);
    }
    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let key = Key::<Aes256Gcm>::from_slice(prf_secret);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(nonce_bytes);
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| PasskeyError::Decrypt)
}
```

- [ ] **Step 5: Register the module**

In `crates/sage/src/lib.rs`, add alongside the other `mod` declarations:

```rust
mod passkey;
```

- [ ] **Step 6: Run tests to verify they pass**

Run:

```bash
cd /Users/don/src/dkackman/sage && export SDKROOT="$(xcrun --show-sdk-path)" && cargo test -p sage passkey::
```

Expected: 3 passed.

- [ ] **Step 7: Commit**

```bash
git add crates/sage/Cargo.toml crates/sage/src/passkey.rs crates/sage/src/lib.rs Cargo.lock
git commit -m "feat(passkey): add AES-GCM password wrap/unwrap keyed by PRF secret"
```

---

### Task 3: PasskeyEnrollment model on Wallet

**Files:**

- Modify: `crates/sage-config/src/wallet.rs`

**Interfaces:**

- Produces: `pub struct PasskeyEnrollment { credential_id: String, rp_id: String, prf_salt: String, wrapped_password: String }` and `Wallet.passkey: Option<PasskeyEnrollment>`.

- [ ] **Step 1: Write the failing tests**

Add to the `mod tests` block in `crates/sage-config/src/wallet.rs`:

```rust
    #[test]
    fn test_passkey_roundtrips_through_toml() {
        let mut wallet = default();
        wallet.passkey = Some(PasskeyEnrollment {
            credential_id: "Y3JlZA".to_string(),
            rp_id: "webauthn.dkackman.com".to_string(),
            prf_salt: "c2FsdA".to_string(),
            wrapped_password: "d3JhcHBlZA==".to_string(),
        });
        let config = WalletConfig {
            defaults: WalletDefaults::default(),
            wallets: vec![wallet],
        };
        let toml = toml::to_string_pretty(&config).unwrap();
        let back: WalletConfig = toml::from_str(&toml).unwrap();
        let enrollment = back.wallets[0].passkey.as_ref().unwrap();
        assert_eq!(enrollment.credential_id, "Y3JlZA");
        assert_eq!(enrollment.rp_id, "webauthn.dkackman.com");
    }

    #[test]
    fn test_passkey_omitted_when_none() {
        let config = WalletConfig {
            defaults: WalletDefaults::default(),
            wallets: vec![default()],
        };
        let toml = toml::to_string_pretty(&config).unwrap();
        assert!(!toml.contains("passkey"));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cd /Users/don/src/dkackman/sage && export SDKROOT="$(xcrun --show-sdk-path)" && cargo test -p sage-config passkey 2>&1 | head -20
```

Expected: FAIL — `PasskeyEnrollment` not found / no field `passkey`.

- [ ] **Step 3: Add the struct**

In `crates/sage-config/src/wallet.rs`, after the `Wallet` struct definition:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PasskeyEnrollment {
    /// WebAuthn credential id (base64url), passed back into allowCredentials.
    pub credential_id: String,
    /// Relying-party id used at enrollment.
    pub rp_id: String,
    /// PRF eval salt (base64url) — must be reused verbatim on unlock.
    pub prf_salt: String,
    /// Standard-base64 of nonce ‖ AES-256-GCM ciphertext of the key's password.
    pub wrapped_password: String,
}
```

- [ ] **Step 4: Add the field**

Add to the `Wallet` struct (after `password_protected`):

```rust
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub passkey: Option<PasskeyEnrollment>,
```

And add `passkey: None,` to `impl Default for Wallet`.

- [ ] **Step 5: Run tests to verify they pass**

Run:

```bash
cd /Users/don/src/dkackman/sage && export SDKROOT="$(xcrun --show-sdk-path)" && cargo test -p sage-config
```

Expected: the two new tests pass AND the pre-existing `test_wallet_config_default` / `test_wallet_config_override` snapshots still pass (a `None` passkey is skipped, so the snapshots are unchanged).

- [ ] **Step 6: Commit**

```bash
git add crates/sage-config/src/wallet.rs
git commit -m "feat(passkey): add PasskeyEnrollment to wallet config"
```

---

### Task 4: Backend endpoints — enroll, unwrap, remove; KeyInfo.passkey; drop-on-change

**Files:**

- Modify: `crates/sage-api/src/requests/keys.rs` (new request/response structs)
- Modify: `crates/sage-api/src/types/key_info.rs` (`PasskeyInfo`, `KeyInfo.passkey`)
- Modify: `crates/sage-api/endpoints.json`
- Modify: `crates/sage/src/error.rs` (new variants + `kind()` arms)
- Modify: `crates/sage/src/endpoints/keys.rs` (methods + `get_key` + `change_password`)
- Modify: `src-tauri/src/lib.rs` (`collect_commands!`)

**Interfaces:**

- Consumes: `crate::passkey::{wrap_password, unwrap_password, PasskeyError}` (Task 2); `sage_config::PasskeyEnrollment` (Task 3).
- Produces: commands `enroll_passkey`, `unwrap_passkey_password`, `remove_passkey`; `KeyInfo.passkey: Option<PasskeyInfo>`.

- [ ] **Step 1: Add the API request/response structs**

In `crates/sage-api/src/requests/keys.rs`, following the exact 3-attribute derive stack used by `ChangePassword`:

```rust
/// Enroll a passkey as an unlock method for a password-protected key
#[cfg_attr(
    feature = "openapi",
    crate::openapi_attr(
        tag = "Authentication & Keys",
        description = "Wrap a key's password under a passkey PRF secret so it can be unlocked by passkey."
    )
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct EnrollPasskey {
    pub fingerprint: u32,
    pub password: String,
    pub credential_id: String,
    pub rp_id: String,
    pub prf_salt: String,
    /// Standard-base64 of the raw 32-byte PRF secret.
    pub prf_secret: String,
}

#[cfg_attr(feature = "openapi", crate::openapi_attr(tag = "Authentication & Keys"))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct EnrollPasskeyResponse {}

/// Unwrap a passkey-enrolled key's password using a fresh PRF secret
#[cfg_attr(
    feature = "openapi",
    crate::openapi_attr(tag = "Authentication & Keys")
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct UnwrapPasskeyPassword {
    pub fingerprint: u32,
    /// Standard-base64 of the raw 32-byte PRF secret from the assertion.
    pub prf_secret: String,
}

#[cfg_attr(feature = "openapi", crate::openapi_attr(tag = "Authentication & Keys"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct UnwrapPasskeyPasswordResponse {
    pub password: String,
}

/// Remove a key's passkey enrollment
#[cfg_attr(feature = "openapi", crate::openapi_attr(tag = "Authentication & Keys"))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct RemovePasskey {
    pub fingerprint: u32,
}

#[cfg_attr(feature = "openapi", crate::openapi_attr(tag = "Authentication & Keys"))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct RemovePasskeyResponse {}
```

(These auto-re-export via `pub use keys::*;` in `crates/sage-api/src/requests.rs`.)

- [ ] **Step 2: Add `PasskeyInfo` and the `KeyInfo.passkey` field**

In `crates/sage-api/src/types/key_info.rs`, add the field to `KeyInfo` (after `emoji`):

```rust
    pub passkey: Option<PasskeyInfo>,
```

And add the struct:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct PasskeyInfo {
    pub credential_id: String,
    pub rp_id: String,
    pub prf_salt: String,
}
```

- [ ] **Step 3: Declare the endpoints (sync) in the manifest**

In `crates/sage-api/endpoints.json`, add near the other key endpoints:

```json
  "enroll_passkey": false,
  "unwrap_passkey_password": false,
  "remove_passkey": false,
```

- [ ] **Step 4: Add error variants**

In `crates/sage/src/error.rs`, add to the `Error` enum:

```rust
    #[error("Passkey error: {0}")]
    Passkey(#[from] crate::passkey::PasskeyError),

    #[error("Base64 decode error: {0}")]
    Base64(#[from] base64::DecodeError),

    #[error("UTF-8 error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),

    #[error("No passkey enrollment for this key")]
    NoPasskeyEnrollment,
```

And add matching arms to `Error::kind()`:

```rust
            Self::Passkey(..) => ErrorKind::IncorrectPassword,
            Self::NoPasskeyEnrollment => ErrorKind::Unauthorized,
            Self::Base64(..) | Self::Utf8(..) => ErrorKind::Internal,
```

(Map `Passkey` — which fires when the PRF secret can't decrypt the wrapped password — to `IncorrectPassword`, consistent with `KeychainError::Decrypt`.)

- [ ] **Step 5: Write the failing endpoint tests**

Append to `crates/sage/src/endpoints/keys.rs` a test module (fields `keychain`, `wallet_config`, `config` are crate-visible, so in-crate tests can populate them):

```rust
#[cfg(test)]
mod passkey_endpoint_tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD, Engine};
    use bip39::Mnemonic;
    use tempfile::tempdir;

    fn sage_with_password_key(password: &[u8]) -> (crate::Sage, u32) {
        let dir = tempdir().unwrap();
        let mut sage = crate::Sage::new(dir.path(), true);
        let mnemonic = Mnemonic::from_entropy(&[7u8; 16]).unwrap();
        let fingerprint = sage.keychain.add_mnemonic(&mnemonic, password).unwrap();
        sage.wallet_config.wallets.push(sage_config::Wallet {
            fingerprint,
            ..Default::default()
        });
        std::mem::forget(dir); // keep temp dir alive for save_config writes
        (sage, fingerprint)
    }

    #[test]
    fn test_enroll_then_unwrap_returns_password() {
        let (mut sage, fingerprint) = sage_with_password_key(b"hunter2");
        let prf = STANDARD.encode([9u8; 32]);
        sage.enroll_passkey(EnrollPasskey {
            fingerprint,
            password: "hunter2".to_string(),
            credential_id: "cred".to_string(),
            rp_id: "webauthn.dkackman.com".to_string(),
            prf_salt: "salt".to_string(),
            prf_secret: prf.clone(),
        })
        .unwrap();

        let out = sage
            .unwrap_passkey_password(UnwrapPasskeyPassword { fingerprint, prf_secret: prf })
            .unwrap();
        assert_eq!(out.password, "hunter2");
    }

    #[test]
    fn test_enroll_rejects_wrong_password() {
        let (mut sage, fingerprint) = sage_with_password_key(b"hunter2");
        let prf = STANDARD.encode([9u8; 32]);
        assert!(sage
            .enroll_passkey(EnrollPasskey {
                fingerprint,
                password: "wrong".to_string(),
                credential_id: "cred".to_string(),
                rp_id: "rp".to_string(),
                prf_salt: "salt".to_string(),
                prf_secret: prf,
            })
            .is_err());
    }

    #[test]
    fn test_change_password_drops_enrollment() {
        let (mut sage, fingerprint) = sage_with_password_key(b"hunter2");
        let prf = STANDARD.encode([9u8; 32]);
        sage.enroll_passkey(EnrollPasskey {
            fingerprint,
            password: "hunter2".to_string(),
            credential_id: "cred".to_string(),
            rp_id: "rp".to_string(),
            prf_salt: "salt".to_string(),
            prf_secret: prf,
        })
        .unwrap();
        sage.change_password(ChangePassword {
            fingerprint,
            old_password: "hunter2".to_string(),
            new_password: "newpass".to_string(),
        })
        .unwrap();
        let wallet = sage
            .wallet_config
            .wallets
            .iter()
            .find(|w| w.fingerprint == fingerprint)
            .unwrap();
        assert!(wallet.passkey.is_none());
    }
}
```

- [ ] **Step 6: Run tests to verify they fail**

Run:

```bash
cd /Users/don/src/dkackman/sage && export SDKROOT="$(xcrun --show-sdk-path)" && cargo test -p sage passkey_endpoint 2>&1 | head -30
```

Expected: FAIL — `enroll_passkey` / `unwrap_passkey_password` not found.

- [ ] **Step 7: Implement the endpoint methods**

In `crates/sage/src/endpoints/keys.rs`, add to the `impl Sage` block. Ensure imports at the top of the file include `use base64::{engine::general_purpose::STANDARD, Engine};` and `use rand::SeedableRng;` and `use rand_chacha::ChaCha20Rng;` (add any missing), plus `EnrollPasskey, EnrollPasskeyResponse, UnwrapPasskeyPassword, UnwrapPasskeyPasswordResponse, RemovePasskey, RemovePasskeyResponse, PasskeyInfo` to the `sage_api` import group.

```rust
    pub fn enroll_passkey(&mut self, req: EnrollPasskey) -> Result<EnrollPasskeyResponse> {
        let password = req.password.into_bytes();

        // Prove the caller knows the current password (also rejects public keys).
        self.keychain.extract_secrets(req.fingerprint, &password)?;

        let prf_secret = STANDARD.decode(&req.prf_secret)?;
        let mut rng = ChaCha20Rng::from_entropy();
        let wrapped_password = crate::passkey::wrap_password(&prf_secret, &password, &mut rng)?;

        let Some(wallet) = self
            .wallet_config
            .wallets
            .iter_mut()
            .find(|wallet| wallet.fingerprint == req.fingerprint)
        else {
            return Err(Error::UnknownFingerprint);
        };

        wallet.passkey = Some(sage_config::PasskeyEnrollment {
            credential_id: req.credential_id,
            rp_id: req.rp_id,
            prf_salt: req.prf_salt,
            wrapped_password,
        });
        self.save_config()?;

        Ok(EnrollPasskeyResponse {})
    }

    pub fn unwrap_passkey_password(
        &self,
        req: UnwrapPasskeyPassword,
    ) -> Result<UnwrapPasskeyPasswordResponse> {
        let Some(wallet) = self
            .wallet_config
            .wallets
            .iter()
            .find(|wallet| wallet.fingerprint == req.fingerprint)
        else {
            return Err(Error::UnknownFingerprint);
        };
        let enrollment = wallet
            .passkey
            .as_ref()
            .ok_or(Error::NoPasskeyEnrollment)?;

        let prf_secret = STANDARD.decode(&req.prf_secret)?;
        let password = crate::passkey::unwrap_password(&prf_secret, &enrollment.wrapped_password)?;

        Ok(UnwrapPasskeyPasswordResponse {
            password: String::from_utf8(password)?,
        })
    }

    pub fn remove_passkey(&mut self, req: RemovePasskey) -> Result<RemovePasskeyResponse> {
        let Some(wallet) = self
            .wallet_config
            .wallets
            .iter_mut()
            .find(|wallet| wallet.fingerprint == req.fingerprint)
        else {
            return Err(Error::UnknownFingerprint);
        };
        wallet.passkey = None;
        self.save_config()?;
        Ok(RemovePasskeyResponse {})
    }
```

- [ ] **Step 8: Populate `KeyInfo.passkey` in `get_key`**

In `get_key` (`crates/sage/src/endpoints/keys.rs`), the `KeyInfo { ... }` literal currently ends at `emoji: wallet_config.emoji`. Add:

```rust
                passkey: wallet_config.passkey.map(|enrollment| PasskeyInfo {
                    credential_id: enrollment.credential_id,
                    rp_id: enrollment.rp_id,
                    prf_salt: enrollment.prf_salt,
                }),
```

(`wallet_config` here is the `.cloned().unwrap_or_default()` local, so `.passkey` is owned.)

- [ ] **Step 9: Drop enrollment on password change/removal**

In `change_password` (`crates/sage/src/endpoints/keys.rs`), after `self.save_keychain()?;` and before `self.set_password_protected(...)`, add:

```rust
        // A wrapped passkey holds the OLD password; changing or removing the
        // password invalidates it, so drop the enrollment (user re-enrolls).
        if let Some(wallet) = self
            .wallet_config
            .wallets
            .iter_mut()
            .find(|wallet| wallet.fingerprint == req.fingerprint)
        {
            wallet.passkey = None;
        }
```

(Key deletion already drops the enrollment: `delete_key` retains `wallets` by fingerprint, discarding the `Wallet` and its `passkey`.)

- [ ] **Step 10: Register the commands**

In `src-tauri/src/lib.rs` `collect_commands![]` (after `commands::change_password,`), add:

```rust
            commands::enroll_passkey,
            commands::unwrap_passkey_password,
            commands::remove_passkey,
```

- [ ] **Step 11: Run tests to verify they pass**

Run:

```bash
cd /Users/don/src/dkackman/sage && export SDKROOT="$(xcrun --show-sdk-path)" && cargo test -p sage passkey_endpoint && cargo check -p sage-tauri
```

Expected: 3 endpoint tests pass; `sage-tauri` compiles (macro-generated Tauri wrappers + RPC routes resolve).

- [ ] **Step 12: Regenerate TypeScript bindings**

Run the app once in debug to rewrite `src/bindings.ts` (this is the only way bindings regenerate):

```bash
cd /Users/don/src/dkackman/sage && export SDKROOT="$(xcrun --show-sdk-path)" && cargo tauri dev
```

Let it boot, confirm `src/bindings.ts` now contains `enrollPasskey`, `unwrapPasskeyPassword`, `removePasskey`, and `PasskeyInfo` / `KeyInfo.passkey`, then quit the app.

- [ ] **Step 13: Commit**

```bash
git add crates/sage-api/src/requests/keys.rs crates/sage-api/src/types/key_info.rs crates/sage-api/endpoints.json crates/sage/src/error.rs crates/sage/src/endpoints/keys.rs src-tauri/src/lib.rs src/bindings.ts
git commit -m "feat(passkey): enroll/unwrap/remove endpoints + KeyInfo.passkey + drop-on-change"
```

---

### Task 5: Frontend — passkey WebAuthn helper, enroll UI, passkey-first unlock

**Files:**

- Create: `src/lib/passkey.ts`
- Modify: `src/contexts/PasswordContext.tsx` (Case 0 + signature)
- Modify: `src/components/WalletCard.tsx:82,180` and `src/pages/Settings.tsx:1237` (call-site signature update)
- Modify: `src/pages/Settings.tsx` (Security section: enroll/remove control)

**Interfaces:**

- Consumes: `commands.enrollPasskey`, `commands.unwrapPasskeyPassword`, `commands.removePasskey`, `KeyInfo.passkey` (Task 4 bindings); `register`/`authenticate` from `tauri-plugin-passkey-api`.
- Produces: `enrollPasskey(fingerprint, password)`, `unlockWithPasskey(info)` in `src/lib/passkey.ts`; a `requestPassword(info)` that tries passkey before the dialog.

- [ ] **Step 1: Write the WebAuthn helper**

Create `src/lib/passkey.ts`:

```ts
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
  const ext = (
    response as {
      clientExtensionResults?: { prf?: { results?: { first?: string } } };
    }
  ).clientExtensionResults;
  return ext?.prf?.results?.first;
}

/** Register a passkey for `fingerprint` and wrap `password` under its PRF secret. */
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
      residentKey: 'discouraged',
      userVerification: 'required',
    },
    attestation: 'none',
    timeout: 60000,
    extensions: { prf: { eval: { first: prfSalt } } },
  });

  const credentialId = created.id;

  const assertion = await authenticate(RP_ORIGIN, {
    challenge: bytesToBase64Url(randomBytes(32)),
    rpId: RP_ID,
    allowCredentials: [{ type: 'public-key', id: credentialId }],
    userVerification: 'required',
    timeout: 60000,
    extensions: { prf: { eval: { first: prfSalt } } },
  });

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
  });

  const prf = prfFirst(assertion);
  if (!prf) throw new Error('Authenticator did not return a PRF secret');

  const result = await commands.unwrapPasskeyPassword({
    fingerprint: info.fingerprint,
    prf_secret: bytesToStdBase64(base64UrlToBytes(prf)),
  });
  return result.password;
}
```

- [ ] **Step 2: Verify types compile**

Run:

```bash
cd /Users/don/src/dkackman/sage && npx tsc -b 2>&1 | head -20
```

Expected: no errors in `src/lib/passkey.ts`. (If the plugin's option types complain about `extensions.prf`, cast the options object with `as PublicKeyCredentialCreationOptionsJSON` / `as PublicKeyCredentialRequestOptionsJSON` — PRF is a valid but sometimes-untyped extension.)

- [ ] **Step 3: Add passkey-first unlock to PasswordContext**

In `src/contexts/PasswordContext.tsx`, change the `requestPassword` signature from `(hasPassword: boolean)` to accept the fields needed for passkey. Update the interface (lines 15-17):

```ts
export interface PasswordContextType {
  requestPassword: (info: {
    has_password: boolean;
    passkey: PasskeyInfo | null;
    fingerprint: number;
  }) => Promise<string | null | undefined>;
}
```

Import `PasskeyInfo` and `KeyInfo` from `@/bindings` and `unlockWithPasskey` from `@/lib/passkey`. At the very top of the `requestPassword` callback (before Case 1 at line 34), add Case 0:

```ts
// Case 0: passkey-enrolled → try passkey first, password dialog as fallback.
if (info.passkey) {
  try {
    return await unlockWithPasskey({
      fingerprint: info.fingerprint,
      passkey: info.passkey,
    } as KeyInfo);
  } catch {
    // fall through to the password dialog
  }
}
```

Then replace the `if (hasPassword)` check on line 34 with `if (info.has_password)`. Update the `useCallback` dependency array as needed.

- [ ] **Step 4: Update the three call sites**

- `src/components/WalletCard.tsx:82`:
  ```ts
  const password = await requestPassword({
    has_password: info.has_password,
    passkey: info.passkey,
    fingerprint: info.fingerprint,
  });
  ```
- `src/components/WalletCard.tsx:180`: same replacement (uses `info`).
- `src/pages/Settings.tsx:1237`:

  ```ts
  const password = await requestPassword({
    has_password: key?.has_password ?? false,
    passkey: key?.passkey ?? null,
    fingerprint: key!.fingerprint,
  });
  ```

- [ ] **Step 5: Add the enroll/remove control to the Security section**

In `src/pages/Settings.tsx`, in the Security section (the `key?.has_secrets` block ending near line 1401), add a control that appears only when the key has a password:

```tsx
{
  key?.has_password &&
    (key.passkey ? (
      <Button
        variant='outline'
        onClick={async () => {
          await commands.removePasskey({ fingerprint: key.fingerprint });
          await handlePasswordSuccess();
        }}
      >
        <Trans>Remove passkey unlock</Trans>
      </Button>
    ) : (
      <Button
        variant='outline'
        onClick={async () => {
          const password = await requestPassword({
            has_password: key.has_password,
            passkey: null,
            fingerprint: key.fingerprint,
          });
          if (typeof password !== 'string') return;
          try {
            await enrollPasskey(key.fingerprint, password);
            await handlePasswordSuccess();
          } catch (e) {
            // surface via the app's existing error handling
            console.error(e);
          }
        }}
      >
        <Trans>Unlock with passkey</Trans>
      </Button>
    ));
}
```

Import `enrollPasskey` from `@/lib/passkey`, `commands` from `@/bindings`, and reuse the existing `requestPassword` (from `usePassword`) and `handlePasswordSuccess` (Settings.tsx:1217). `handlePasswordSuccess` already re-fetches `getKey`, refreshing `key.passkey`.

- [ ] **Step 6: Verify frontend compiles and lints**

Run:

```bash
cd /Users/don/src/dkackman/sage && npx tsc -b && pnpm lint
```

Expected: no type errors, no new lint errors.

- [ ] **Step 7: Extract i18n strings**

Run:

```bash
cd /Users/don/src/dkackman/sage && pnpm extract
```

Expected: the two new `<Trans>` strings appear in the catalogs.

- [ ] **Step 8: Commit**

```bash
git add src/lib/passkey.ts src/contexts/PasswordContext.tsx src/components/WalletCard.tsx src/pages/Settings.tsx src/locales
git commit -m "feat(passkey): WebAuthn helper, enroll UI, passkey-first unlock"
```

---

### Task 6: macOS entitlements, AASA, and dev-bundle signing

**Files:**

- Create: `src-tauri/Entitlements.plist`
- Create: `build-macos-dev.sh` (adapted from the plugin test-app)
- External: AASA on `webauthn.dkackman.com` (Cloudflare Worker); Apple Developer portal (App ID + provisioning profile)

**Interfaces:**

- Produces: a signed `Sage.app` whose entitlements permit platform-authenticator WebAuthn against `webauthn.dkackman.com`.

- [ ] **Step 1: Add sage's App ID to the AASA**

In the Cloudflare Worker serving `https://webauthn.dkackman.com/.well-known/apple-app-site-association`, add sage's App ID to the `webcredentials.apps` array:

```
86TDY6D9V2.com.rigidnetwork.sage
```

Verify:

```bash
curl -s https://webauthn.dkackman.com/.well-known/apple-app-site-association | jq '.webcredentials'
```

Expected: the array includes `86TDY6D9V2.com.rigidnetwork.sage`.

- [ ] **Step 2: Register the App ID + provisioning profile (Apple Developer portal)**

At https://developer.apple.com/account/resources/identifiers/list register/enable **Associated Domains** for App ID `com.rigidnetwork.sage`. Then create a **macOS App Development** provisioning profile for that App ID + your dev certificate + this Mac, download it, and place it at `src-tauri/embedded.provisionprofile`.

- [ ] **Step 3: Create the entitlements file**

Create `src-tauri/Entitlements.plist` (modeled on the plugin test-app's):

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>com.apple.security.get-task-allow</key>
    <true/>
    <key>com.apple.application-identifier</key>
    <string>86TDY6D9V2.com.rigidnetwork.sage</string>
    <key>com.apple.developer.team-identifier</key>
    <string>86TDY6D9V2</string>
    <key>com.apple.developer.associated-domains</key>
    <array>
        <string>webcredentials:webauthn.dkackman.com?mode=developer</string>
    </array>
</dict>
</plist>
```

- [ ] **Step 4: Adapt the dev-signing script**

Copy `/Users/don/src/dkackman/tauri-plugin-passkey/test-app/build-macos-dev.sh` to sage root as `build-macos-dev.sh`. It already derives `BUNDLE_ID`/`APP_NAME` from `src-tauri/tauri.conf.json` and `TEAM_ID` from the entitlements, and expects `src-tauri/Entitlements.plist` + `embedded.provisionprofile`. Adjust only paths that differ (sage builds via `pnpm tauri build --debug`; the app bundle lands at `src-tauri/target/debug/bundle/macos/Sage.app`). Make it executable: `chmod +x build-macos-dev.sh`.

- [ ] **Step 5: Build and sign**

Run:

```bash
cd /Users/don/src/dkackman/sage && export SDKROOT="$(xcrun --show-sdk-path)" && ./build-macos-dev.sh
```

Expected: a signed `Sage.app`. Verify the entitlements are embedded:

```bash
codesign -d --entitlements :- src-tauri/target/debug/bundle/macos/Sage.app 2>/dev/null | grep -A1 associated-domains
```

Expected: shows `webcredentials:webauthn.dkackman.com`.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/Entitlements.plist build-macos-dev.sh .gitignore
git commit -m "chore(passkey): macOS entitlements + dev-bundle signing for WebAuthn"
```

(Add `src-tauri/embedded.provisionprofile` to `.gitignore` — it is developer-specific and must not be committed.)

---

### Task 7: Manual end-to-end verification on macOS

**Files:** none (verification only).

- [ ] **Step 1: Launch the signed bundle**

```bash
open /Users/don/src/dkackman/sage/src-tauri/target/debug/bundle/macos/Sage.app
```

(Running the raw binary via `cargo tauri dev` will NOT work — ASAuthorization requires the signed bundle. Rebuild+sign with `./build-macos-dev.sh` after any change you want to test here.)

- [ ] **Step 2: Enroll**

Create or open a key that has a password. In Settings → the key's Security section, click **Unlock with passkey**, enter the current password when prompted, and complete the Touch ID prompt. Expected: no error; the button switches to **Remove passkey unlock**. Confirm `wallets.toml` in the app data dir now has a `[wallets.passkey]` entry for that fingerprint with a `wrapped_password`.

- [ ] **Step 3: Unlock by passkey**

Trigger an action that requires the password (e.g. view secret key, or delete-key confirm). Expected: a Touch ID prompt appears instead of the password dialog; approving it unlocks. Confirm the operation succeeds with the recovered password.

- [ ] **Step 4: Fallback**

Trigger the same action again and CANCEL the Touch ID prompt. Expected: the app falls through to the typed-password dialog, and entering the correct password still works.

- [ ] **Step 5: Stale-password rule**

Change the key's password (Settings → Change). Expected: the Security section shows **Unlock with passkey** again (enrollment dropped). Re-enroll to confirm the loop.

- [ ] **Step 6: Record the outcome**

Note any plugin integration issues discovered (this is the pre-publish shakeout). File them against `../../tauri-plugin-passkey` and, if a plugin change is needed, branch it there per the spec.

---

## Self-Review

**Spec coverage:**

- Plugin wiring (spec Increment 1) → Task 1. ✓
- Rust core: model, config, wrap/unwrap, endpoints, tests (Increment 2) → Tasks 2, 3, 4. ✓
- macOS enablement (Increment 3) → Task 6. ✓
- Frontend enroll + Case 0 + drop-on-change (Increment 4) → Tasks 4 (drop-on-change backend) + 5 (UI/unlock). ✓
- Manual E2E (Increment 5) → Task 7. ✓
- No RP server → helper generates challenges locally, no verification (Task 5). ✓
- Storage in config not keychain → Task 3 (`sage-config`), keychain untouched. ✓
- Biometric untouched → no task modifies `BiometricContext`; Case 0 sits above the existing cases. ✓
- Stale-password rule → Task 4 Step 9 (change/remove) + delete already handled. ✓

**Type consistency:** `EnrollPasskey`/`UnwrapPasskeyPassword`/`RemovePasskey` (+ `Response`) and `PasskeyInfo`/`PasskeyEnrollment` field names (`credential_id`, `rp_id`, `prf_salt`, `wrapped_password`, `prf_secret`) are identical across the API structs (Task 4 Step 1), the config struct (Task 3), the endpoint bodies (Task 4 Step 7), and the frontend calls (Task 5). `commands.enrollPasskey` / `unwrapPasskeyPassword` / `removePasskey` match the snake_case endpoint names via tauri-specta camelCasing.

**Placeholder scan:** no TBD/TODO; every code step has concrete code; every test step has real assertions and a run command with expected output.
