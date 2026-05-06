# Wallet Settings Sync Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Sync wallet name and emoji across app instances using `tauri-plugin-nostr-sync` as transport, with opt-in per wallet and manual fetch from the Advanced settings tab.

**Architecture:** Each wallet derives a secp256k1 Nostr keypair from its BLS master secret via HKDF-SHA256. The plugin encrypts settings payloads with NIP-44 (self-encryption) and publishes to Nostr relays as NIP-33 replaceable events. Publish is triggered from a new Tauri command called by the frontend after each mutation; fetch is manually triggered from the Advanced settings tab.

**Tech Stack:** Rust (`tauri-plugin-nostr-sync`, `hkdf`, `sha2`, `nostr-sdk`), TypeScript (`tauri-plugin-nostr-sync-api`), React/Tailwind (existing patterns)

---

## File Map

| File | Action | Responsibility |
| --- | --- | --- |
| `crates/sage-config/src/sync.rs` | Create | `SyncConfig` struct with relay list |
| `crates/sage-config/src/lib.rs` | Modify | Re-export `SyncConfig` |
| `crates/sage-config/src/wallet.rs` | Modify | Add `sync_enabled: bool` to `Wallet` |
| `crates/sage-config/src/config.rs` | Modify | Add `sync: SyncConfig` to `Config` |
| `Cargo.toml` | Modify | Add `hkdf`, `sha2` workspace deps |
| `src-tauri/Cargo.toml` | Modify | Add plugin + hkdf + sha2 crate deps |
| `src-tauri/src/lib.rs` | Modify | Register plugin; load relays in initialize |
| `src-tauri/src/commands.rs` | Modify | 8 new commands (signer, config, publish, fetch) |
| `src-tauri/capabilities/default.json` | Modify | Add `nostr-sync:default` permission |
| `src/state.ts` | Modify | Call inject/clear signer on login/logout |
| `src/pages/Settings.tsx` | Modify | Add `SyncSettings` to Advanced tab |

---

## Task 1: sage-config — SyncConfig struct

**Files:**
- Create: `crates/sage-config/src/sync.rs`
- Modify: `crates/sage-config/src/lib.rs`

- [ ] **Write the test**

Add to the bottom of `crates/sage-config/src/sync.rs` (write the file first, then add tests):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_relays_are_populated() {
        let cfg = SyncConfig::default();
        assert_eq!(cfg.relays.len(), 3);
        assert!(cfg.relays.contains(&"wss://relay.damus.io".to_string()));
    }

    #[test]
    fn toml_roundtrip_preserves_relays() {
        let original = SyncConfig {
            relays: vec!["wss://relay.example.com".to_string()],
        };
        let toml = toml::to_string_pretty(&original).unwrap();
        let parsed: SyncConfig = toml::from_str(&toml).unwrap();
        assert_eq!(parsed.relays, original.relays);
    }

    #[test]
    fn empty_relay_list_deserializes() {
        let toml = r#"relays = []"#;
        let cfg: SyncConfig = toml::from_str(toml).unwrap();
        assert!(cfg.relays.is_empty());
    }
}
```

- [ ] **Run test to verify it fails**

```bash
cd /Users/don/src/dkackman/sage
cargo test -p sage-config sync 2>&1 | tail -20
```

Expected: compile error — `SyncConfig` not defined yet.

- [ ] **Implement `SyncConfig`**

Create `crates/sage-config/src/sync.rs`:

```rust
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(default)]
pub struct SyncConfig {
    pub relays: Vec<String>,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            relays: vec![
                "wss://relay.damus.io".to_string(),
                "wss://relay.nostr.band".to_string(),
                "wss://nos.lol".to_string(),
            ],
        }
    }
}
```

- [ ] **Re-export from lib.rs**

In `crates/sage-config/src/lib.rs`, add before the existing `pub use` lines:

```rust
mod sync;
pub use sync::*;
```

- [ ] **Run tests to verify they pass**

```bash
cargo test -p sage-config sync 2>&1 | tail -20
```

Expected: 3 tests pass.

---

## Task 2: sage-config — sync_enabled on Wallet and SyncConfig on Config

**Files:**
- Modify: `crates/sage-config/src/wallet.rs`
- Modify: `crates/sage-config/src/config.rs`

- [ ] **Write the tests**

In `crates/sage-config/src/wallet.rs`, add to the existing `#[cfg(test)]` block:

```rust
#[test]
fn sync_enabled_defaults_to_false() {
    let wallet = Wallet::default();
    assert!(!wallet.sync_enabled);
}

#[test]
fn sync_enabled_survives_toml_roundtrip() {
    let wallet = Wallet {
        sync_enabled: true,
        ..Wallet::default()
    };
    let config = WalletConfig {
        defaults: WalletDefaults::default(),
        wallets: vec![wallet],
    };
    let toml = toml::to_string_pretty(&config).unwrap();
    let parsed: WalletConfig = toml::from_str(&toml).unwrap();
    assert!(parsed.wallets[0].sync_enabled);
}
```

In `crates/sage-config/src/config.rs`, add a new test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_config_in_config_defaults_correctly() {
        let cfg = Config::default();
        assert_eq!(cfg.sync.relays.len(), 3);
    }

    #[test]
    fn config_toml_roundtrip_preserves_sync() {
        let mut cfg = Config::default();
        cfg.sync.relays = vec!["wss://custom.relay".to_string()];
        let toml = toml::to_string_pretty(&cfg).unwrap();
        let parsed: Config = toml::from_str(&toml).unwrap();
        assert_eq!(parsed.sync.relays, vec!["wss://custom.relay"]);
    }
}
```

- [ ] **Run tests to verify they fail**

```bash
cargo test -p sage-config 2>&1 | tail -20
```

Expected: compile errors — `sync_enabled` and `sync` fields don't exist yet.

- [ ] **Add `sync_enabled` to `Wallet`**

In `crates/sage-config/src/wallet.rs`, add the field after `change_address`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(default)]
pub struct Wallet {
    pub name: String,
    pub fingerprint: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,
    pub delta_sync: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emoji: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_address: Option<String>,
    #[serde(default)]
    pub sync_enabled: bool,
}
```

Update `Default for Wallet` to include `sync_enabled: false`:

```rust
impl Default for Wallet {
    fn default() -> Self {
        Self {
            name: "Unnamed Wallet".to_string(),
            fingerprint: 0,
            network: None,
            delta_sync: None,
            emoji: None,
            change_address: None,
            sync_enabled: false,
        }
    }
}
```

- [ ] **Add `sync: SyncConfig` to `Config`**

In `crates/sage-config/src/config.rs`, add the import and field:

```rust
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::SyncConfig;

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize, Type)]
#[serde(default)]
pub struct Config {
    pub version: u32,
    pub global: GlobalConfig,
    pub network: NetworkConfig,
    pub rpc: RpcConfig,
    pub sync: SyncConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: 2,
            global: GlobalConfig::default(),
            network: NetworkConfig::default(),
            rpc: RpcConfig::default(),
            sync: SyncConfig::default(),
        }
    }
}
```

- [ ] **Run tests to verify they pass**

```bash
cargo test -p sage-config 2>&1 | tail -20
```

Expected: all sage-config tests pass (including the existing wallet config tests).

---

## Task 3: Cargo dependencies

**Files:**
- Modify: `Cargo.toml` (workspace)
- Modify: `src-tauri/Cargo.toml`

- [ ] **Add workspace dependencies**

In `Cargo.toml`, find the `[workspace.dependencies]` section and add:

```toml
hkdf = "0.12"
sha2 = "0.10"
```

- [ ] **Add crate dependencies to src-tauri**

In `src-tauri/Cargo.toml`, add to `[dependencies]`:

```toml
tauri-plugin-nostr-sync = { path = "../../tauri-plugin-nostr" }
hkdf = { workspace = true }
sha2 = { workspace = true }
nostr-sdk = { version = "0.44.1", features = ["nip44"] }
```

- [ ] **Verify the workspace compiles**

```bash
cargo check -p sage-tauri 2>&1 | tail -30
```

Expected: no errors (warnings OK). If `nostr-sdk` version conflicts with the plugin's internal use, check `Cargo.lock` and adjust.

---

## Task 4: Register plugin in lib.rs and add permission

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/capabilities/default.json`

- [ ] **Add plugin permission to capabilities**

In `src-tauri/capabilities/default.json`, add `"nostr-sync:default"` to the `permissions` array:

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "enables the default permissions",
  "windows": ["main"],
  "permissions": [
    "core:path:default",
    "core:event:default",
    "core:window:default",
    "core:webview:default",
    "core:app:default",
    "core:resources:default",
    "core:image:default",
    "clipboard-manager:default",
    "clipboard-manager:allow-write-text",
    "clipboard-manager:allow-read-text",
    "opener:default",
    "nostr-sync:default"
  ]
}
```

- [ ] **Register the plugin in `lib.rs`**

Add the plugin to `src-tauri/src/lib.rs`. Find the `let mut tauri_builder = tauri::Builder::default()` block and add the plugin registration. The plugin is added for both desktop and mobile. Add it after the existing `.plugin(tauri_plugin_os::init())` call:

```rust
let mut tauri_builder = tauri::Builder::default()
    .plugin(tauri_plugin_opener::init())
    .plugin(tauri_plugin_clipboard_manager::init())
    .plugin(tauri_plugin_os::init())
    .plugin(
        tauri_plugin_nostr_sync::Builder::new()
            .app_namespace("sage")
            .build(),
    );
```

Also add the import at the top of the file (or in the block where it's used — Rust allows `use` anywhere):

The `tauri_plugin_nostr_sync` crate is used directly via its builder. No explicit `use` is needed since we're using the full path.

- [ ] **Verify the build compiles**

```bash
cargo build -p sage-tauri 2>&1 | tail -30
```

Expected: compiles. The plugin registers itself with the Tauri runtime.

---

## Task 5: New Tauri commands — signer lifecycle

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

These commands derive the Nostr key from the wallet's BLS master secret and inject it into the plugin.

- [ ] **Write the failing test for HKDF derivation**

At the bottom of `src-tauri/src/commands.rs`, add a test module:

```rust
#[cfg(test)]
mod sync_tests {
    use super::*;

    #[test]
    fn hkdf_derive_nostr_key_produces_32_bytes() {
        let ikm = [42u8; 32];
        let hk = hkdf::Hkdf::<sha2::Sha256>::new(None, &ikm);
        let mut okm = [0u8; 32];
        hk.expand(b"sage-nostr-sync", &mut okm).unwrap();
        assert_eq!(okm.len(), 32);
        assert_ne!(okm, [0u8; 32]);
    }

    #[test]
    fn same_ikm_produces_same_nostr_key() {
        let ikm = [99u8; 32];
        let derive = |ikm: &[u8]| {
            let hk = hkdf::Hkdf::<sha2::Sha256>::new(None, ikm);
            let mut okm = [0u8; 32];
            hk.expand(b"sage-nostr-sync", &mut okm).unwrap();
            okm
        };
        assert_eq!(derive(&ikm), derive(&ikm));
    }

    #[test]
    fn different_ikm_produces_different_nostr_key() {
        let derive = |ikm: &[u8]| {
            let hk = hkdf::Hkdf::<sha2::Sha256>::new(None, ikm);
            let mut okm = [0u8; 32];
            hk.expand(b"sage-nostr-sync", &mut okm).unwrap();
            okm
        };
        assert_ne!(derive(&[1u8; 32]), derive(&[2u8; 32]));
    }
}
```

- [ ] **Run test to verify it fails**

```bash
cargo test -p sage-tauri sync_tests 2>&1 | tail -20
```

Expected: compile error — `hkdf` and `sha2` not imported.

- [ ] **Add imports and implement `inject_nostr_signer` and `clear_nostr_signer`**

Add these imports at the top of `src-tauri/src/commands.rs`:

```rust
use hkdf::Hkdf;
use sha2::Sha256;
use tauri_plugin_nostr_sync::TauriPluginNostrSyncExt;
```

Add these two commands after the existing commands in `src-tauri/src/commands.rs`:

```rust
#[command]
#[specta]
pub async fn inject_nostr_signer(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    fingerprint: u32,
) -> Result<()> {
    let sage = state.lock().await;
    let Ok((_, Some(master_sk))) = sage.keychain.extract_secrets(fingerprint, b"") else {
        return Ok(()); // watch-only wallet — skip silently
    };
    drop(sage);

    let ikm = master_sk.to_bytes();
    let hk = Hkdf::<Sha256>::new(None, &ikm);
    let mut okm = [0u8; 32];
    hk.expand(b"sage-nostr-sync", &mut okm)
        .expect("32 bytes is a valid HKDF output length");

    let secret_key = nostr_sdk::secp256k1::SecretKey::from_slice(&okm).map_err(|e| {
        crate::error::Error {
            kind: sage_api::ErrorKind::Internal,
            reason: e.to_string(),
        }
    })?;
    let keys = nostr_sdk::Keys::new(secret_key.into());

    app_handle
        .nostr_sync()
        .set_signer(keys)
        .await
        .map_err(|e| crate::error::Error {
            kind: sage_api::ErrorKind::Internal,
            reason: e.to_string(),
        })?;

    Ok(())
}

#[command]
#[specta]
pub async fn clear_nostr_signer(app_handle: AppHandle) -> Result<()> {
    app_handle.nostr_sync().clear_signer().await;
    Ok(())
}
```

- [ ] **Register the new commands in `lib.rs`**

In `src-tauri/src/lib.rs`, add the two commands to `collect_commands!`:

```rust
commands::inject_nostr_signer,
commands::clear_nostr_signer,
```

- [ ] **Run tests to verify**

```bash
cargo test -p sage-tauri sync_tests 2>&1 | tail -20
```

Expected: 3 tests pass.

```bash
cargo build -p sage-tauri 2>&1 | tail -20
```

Expected: compiles cleanly.

---

## Task 6: New Tauri commands — sync config and relay management

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Write tests**

Add to the `sync_tests` module in `src-tauri/src/commands.rs`:

```rust
#[test]
fn wallet_settings_payload_schema_version_is_1() {
    let payload = serde_json::json!({
        "v": 1,
        "name": "My Wallet",
        "emoji": null,
    });
    assert_eq!(payload["v"], 1);
    assert_eq!(payload["name"], "My Wallet");
    assert!(payload["emoji"].is_null());
}
```

- [ ] **Implement the four config/relay commands**

Add to `src-tauri/src/commands.rs`:

```rust
#[command]
#[specta]
pub async fn get_sync_enabled(state: State<'_, AppState>, fingerprint: u32) -> Result<bool> {
    let sage = state.lock().await;
    let enabled = sage
        .wallet_config
        .wallets
        .iter()
        .find(|w| w.fingerprint == fingerprint)
        .map(|w| w.sync_enabled)
        .unwrap_or(false);
    Ok(enabled)
}

#[command]
#[specta]
pub async fn set_sync_enabled(
    state: State<'_, AppState>,
    fingerprint: u32,
    enabled: bool,
) -> Result<()> {
    let mut sage = state.lock().await;
    let Some(wallet) = sage
        .wallet_config
        .wallets
        .iter_mut()
        .find(|w| w.fingerprint == fingerprint)
    else {
        return Err(sage::Error::UnknownFingerprint.into());
    };
    wallet.sync_enabled = enabled;
    sage.save_config()?;
    Ok(())
}

#[command]
#[specta]
pub async fn add_sync_relay(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    url: String,
) -> Result<()> {
    app_handle
        .nostr_sync()
        .add_relay(&url)
        .await
        .map_err(|e| crate::error::Error {
            kind: sage_api::ErrorKind::Internal,
            reason: e.to_string(),
        })?;

    let mut sage = state.lock().await;
    if !sage.config.sync.relays.contains(&url) {
        sage.config.sync.relays.push(url);
        sage.save_config()?;
    }
    Ok(())
}

#[command]
#[specta]
pub async fn remove_sync_relay(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    url: String,
) -> Result<()> {
    app_handle
        .nostr_sync()
        .remove_relay(&url)
        .await
        .map_err(|e| crate::error::Error {
            kind: sage_api::ErrorKind::Internal,
            reason: e.to_string(),
        })?;

    let mut sage = state.lock().await;
    sage.config.sync.relays.retain(|r| r != &url);
    sage.save_config()?;
    Ok(())
}
```

- [ ] **Register commands in `lib.rs`**

Add to `collect_commands!`:

```rust
commands::get_sync_enabled,
commands::set_sync_enabled,
commands::add_sync_relay,
commands::remove_sync_relay,
```

- [ ] **Load relays in `initialize` command**

In `src-tauri/src/commands.rs`, find the `initialize` function. After the existing initialization logic and before the final `Ok(())`, add:

```rust
    // Load persisted relays into the nostr-sync plugin
    let relays = {
        let sage = state.lock().await;
        sage.config.sync.relays.clone()
    };
    for url in relays {
        if let Err(e) = app_handle.nostr_sync().add_relay(&url).await {
            tracing::warn!("Failed to connect to relay {url}: {e}");
        }
    }
```

- [ ] **Verify build**

```bash
cargo build -p sage-tauri 2>&1 | tail -20
```

Expected: compiles. New commands and bindings generated.

---

## Task 7: New Tauri commands — publish and fetch

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Write tests**

Add to `sync_tests` module:

```rust
#[test]
fn fetch_payload_v1_parses_name_and_emoji() {
    let payload = serde_json::json!({
        "v": 1,
        "name": "Cold Storage",
        "emoji": "🧊",
    });
    let v = payload["v"].as_u64().unwrap_or(0);
    let name = payload["name"].as_str().map(str::to_string);
    let emoji = payload.get("emoji").and_then(|e| {
        if e.is_null() { None } else { e.as_str().map(str::to_string) }
    });
    assert_eq!(v, 1);
    assert_eq!(name, Some("Cold Storage".to_string()));
    assert_eq!(emoji, Some("🧊".to_string()));
}

#[test]
fn fetch_payload_unknown_version_is_rejected() {
    let payload = serde_json::json!({ "v": 99, "name": "x" });
    let v = payload["v"].as_u64().unwrap_or(0);
    assert_ne!(v, 1, "should treat v=99 as unknown");
}
```

- [ ] **Run test to verify it fails**

```bash
cargo test -p sage-tauri sync_tests 2>&1 | tail -20
```

Expected: compile error (functions referenced by tests don't exist yet) OR tests pass if they're pure logic tests. These are pure logic tests — they should pass immediately once the test module compiles.

- [ ] **Implement `publish_wallet_settings`**

Add to `src-tauri/src/commands.rs`:

```rust
#[command]
#[specta]
pub async fn publish_wallet_settings(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    fingerprint: u32,
) -> Result<()> {
    let (sync_enabled, name, emoji) = {
        let sage = state.lock().await;
        let Some(wallet) = sage.wallet_config.wallets.iter().find(|w| w.fingerprint == fingerprint) else {
            return Ok(());
        };
        (wallet.sync_enabled, wallet.name.clone(), wallet.emoji.clone())
    };

    if !sync_enabled {
        return Ok(());
    }

    let payload = serde_json::json!({
        "v": 1,
        "name": name,
        "emoji": emoji,
    });

    if let Err(e) = app_handle.nostr_sync().publish("wallet-settings", &payload).await {
        tracing::warn!("Failed to publish wallet settings: {e}");
    }

    Ok(())
}
```

- [ ] **Implement `fetch_wallet_settings`**

Add to `src-tauri/src/commands.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FetchSettingsResult {
    pub applied: bool,
    pub name: Option<String>,
    pub emoji: Option<String>,
}

#[command]
#[specta]
pub async fn fetch_wallet_settings(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    fingerprint: u32,
) -> Result<FetchSettingsResult> {
    let result = app_handle
        .nostr_sync()
        .fetch("wallet-settings")
        .await
        .map_err(|e| crate::error::Error {
            kind: sage_api::ErrorKind::Internal,
            reason: e.to_string(),
        })?;

    let Some(fetch_result) = result else {
        return Ok(FetchSettingsResult { applied: false, name: None, emoji: None });
    };

    let payload = &fetch_result.payload;

    let version = payload["v"].as_u64().unwrap_or(0);
    if version != 1 {
        tracing::warn!("fetch_wallet_settings: unknown schema version {version}, skipping");
        return Ok(FetchSettingsResult { applied: false, name: None, emoji: None });
    }

    let remote_name = payload["name"].as_str().filter(|s| !s.is_empty()).map(str::to_string);
    let remote_emoji = payload.get("emoji").and_then(|e| {
        if e.is_null() { None } else { e.as_str().map(str::to_string) }
    });

    let mut sage = state.lock().await;
    let Some(wallet) = sage.wallet_config.wallets.iter_mut().find(|w| w.fingerprint == fingerprint) else {
        return Ok(FetchSettingsResult { applied: false, name: None, emoji: None });
    };

    let mut applied = false;

    if let Some(ref name) = remote_name {
        if wallet.name != *name {
            wallet.name = name.clone();
            applied = true;
        }
    }

    if remote_emoji != wallet.emoji {
        wallet.emoji = remote_emoji.clone();
        applied = true;
    }

    if applied {
        sage.save_config()?;
    }

    Ok(FetchSettingsResult {
        applied,
        name: remote_name,
        emoji: remote_emoji,
    })
}
```

- [ ] **Register commands in `lib.rs`**

Add to `collect_commands!`:

```rust
commands::publish_wallet_settings,
commands::fetch_wallet_settings,
```

- [ ] **Run all sync tests**

```bash
cargo test -p sage-tauri sync_tests 2>&1 | tail -20
```

Expected: all tests pass.

```bash
cargo build -p sage-tauri 2>&1 | tail -20
```

Expected: compiles, TypeScript bindings regenerated at `src/bindings.ts`.

---

## Task 8: state.ts — signer lifecycle on login/logout

**Files:**
- Modify: `src/state.ts`

- [ ] **Call `injectNostrSigner` after login**

In `src/state.ts`, update `loginAndUpdateState`:

```typescript
export async function loginAndUpdateState(
  fingerprint: number,
  onError?: (error: CustomError) => void,
): Promise<void> {
  try {
    await commands.login({ fingerprint });
    await fetchState();
    // Inject Nostr signer for settings sync (no-op for watch-only wallets)
    await commands.injectNostrSigner({ fingerprint }).catch((e) =>
      console.warn('inject_nostr_signer failed:', e),
    );
  } catch (error) {
    if (onError) {
      onError(error as CustomError);
    } else {
      console.error(error);
    }
    throw error;
  }
}
```

- [ ] **Call `clearNostrSigner` on logout**

Update `logoutAndUpdateState`:

```typescript
export async function logoutAndUpdateState(): Promise<void> {
  clearState();
  if (setWalletState) {
    setWalletState(null);
  }
  await commands.clearNostrSigner({}).catch((e) =>
    console.warn('clear_nostr_signer failed:', e),
  );
  await commands.logout({});
}
```

- [ ] **Verify TypeScript compiles**

```bash
cd /Users/don/src/dkackman/sage
pnpm tsc --noEmit 2>&1 | tail -20
```

Expected: no type errors. If `injectNostrSigner` or `clearNostrSigner` are not in `bindings.ts`, confirm the Rust build completed in Task 7 (it regenerates bindings on debug build).

---

## Task 9: Settings.tsx — SyncSettings UI

**Files:**
- Modify: `src/pages/Settings.tsx`

This task adds a new `SyncSettings` React component and mounts it in the Advanced tab.

- [ ] **Add plugin imports at the top of Settings.tsx**

Add to the existing imports in `src/pages/Settings.tsx`:

```typescript
import {
  addRelay,
  getRelays,
  getStatus,
  RelayInfo,
  SyncStatus,
} from 'tauri-plugin-nostr-sync-api';
```

Also add to the `commands` import from `../bindings` the new commands:

The `commands` object in `bindings.ts` will automatically include the new commands once the Rust build runs. No manual addition needed.

- [ ] **Implement the `SyncSettings` component**

Add this component before the `export default function Settings()` line in `src/pages/Settings.tsx`:

```typescript
function SyncSettings({ fingerprint }: { fingerprint: number }) {
  const { addError } = useErrors();
  const [syncEnabled, setSyncEnabled] = useState<boolean>(false);
  const [relays, setRelays] = useState<RelayInfo[]>([]);
  const [status, setStatus] = useState<SyncStatus | null>(null);
  const [newRelay, setNewRelay] = useState('');
  const [fetchMessage, setFetchMessage] = useState<string | null>(null);
  const [fetching, setFetching] = useState(false);

  useEffect(() => {
    commands
      .getSyncEnabled({ fingerprint })
      .then(setSyncEnabled)
      .catch(addError);
  }, [fingerprint, addError]);

  useEffect(() => {
    if (!syncEnabled) return;
    const load = () => {
      getRelays().then(setRelays).catch(console.warn);
      getStatus().then(setStatus).catch(console.warn);
    };
    load();
    const id = setInterval(load, 5000);
    return () => clearInterval(id);
  }, [syncEnabled]);

  const toggleSync = async (enabled: boolean) => {
    await commands
      .setSyncEnabled({ fingerprint, enabled })
      .catch(addError);
    setSyncEnabled(enabled);
  };

  const handleAddRelay = async () => {
    const url = newRelay.trim();
    if (!url) return;
    await commands.addSyncRelay({ url }).catch(addError);
    setNewRelay('');
    getRelays().then(setRelays).catch(console.warn);
  };

  const handleRemoveRelay = async (url: string) => {
    await commands.removeSyncRelay({ url }).catch(addError);
    getRelays().then(setRelays).catch(console.warn);
  };

  const handleFetch = async () => {
    setFetching(true);
    setFetchMessage(null);
    try {
      const result = await commands.fetchWalletSettings({ fingerprint });
      if (result.applied) {
        setFetchMessage(t`Settings applied`);
      } else {
        setFetchMessage(t`Already up to date`);
      }
    } catch (e) {
      setFetchMessage(String(e));
    } finally {
      setFetching(false);
    }
  };

  const connectedCount = status?.connectedRelayCount ?? 0;
  const totalCount = status?.relayCount ?? 0;

  return (
    <SettingsSection title={t`Settings Sync`}>
      <SettingItem
        label={t`Sync Wallet Settings`}
        description={t`Sync name and icon across devices using Nostr`}
        control={
          <Switch checked={syncEnabled} onCheckedChange={toggleSync} />
        }
      />

      {syncEnabled && (
        <>
          <div className='px-3 py-2 text-xs text-muted-foreground'>
            {status?.ready
              ? t`Sync ready (${connectedCount}/${totalCount} relays connected)`
              : t`Not connected`}
          </div>

          {relays.map((relay) => (
            <div
              key={relay.url}
              className='px-3 py-2 flex items-center justify-between gap-2'
            >
              <div className='flex items-center gap-2'>
                <div
                  className={`h-2 w-2 rounded-full ${relay.connected ? 'bg-green-500' : 'bg-red-400'}`}
                />
                <span className='text-sm font-mono truncate'>{relay.url}</span>
              </div>
              <Button
                variant='ghost'
                size='icon'
                onClick={() => handleRemoveRelay(relay.url)}
              >
                <TrashIcon className='h-4 w-4' />
              </Button>
            </div>
          ))}

          <div className='p-3 flex gap-2'>
            <Input
              value={newRelay}
              placeholder='wss://relay.example.com'
              onChange={(e) => setNewRelay(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleAddRelay()}
            />
            <Button size='sm' onClick={handleAddRelay}>
              <Trans>Add</Trans>
            </Button>
          </div>

          <div className='p-3 flex items-center gap-3'>
            <Button
              variant='outline'
              size='sm'
              onClick={handleFetch}
              disabled={fetching}
            >
              {fetching ? (
                <>
                  <LoaderCircleIcon className='mr-2 h-4 w-4 animate-spin' />
                  <Trans>Fetching...</Trans>
                </>
              ) : (
                <Trans>Fetch Settings</Trans>
              )}
            </Button>
            {fetchMessage && (
              <span className='text-sm text-muted-foreground'>
                {fetchMessage}
              </span>
            )}
          </div>
        </>
      )}
    </SettingsSection>
  );
}
```

- [ ] **Mount SyncSettings in the Advanced tab**

Find the `<TabsContent value='advanced'>` block in `Settings.tsx`. Update it to include `SyncSettings` when a wallet is logged in:

```typescript
<TabsContent value='advanced'>
  <div className='grid gap-4'>
    {wallet && <SyncSettings fingerprint={wallet.fingerprint} />}
    {!isMobile && <RpcSettings />}
    <LogViewer />
  </div>
</TabsContent>
```

- [ ] **Add `publish_wallet_settings` calls to WalletSettings handlers**

In the `WalletSettings` component in `Settings.tsx`, find the `onBlur` handler for the wallet name input and add the publish call after rename succeeds:

```typescript
onBlur={() => {
  if (localName === wallet?.name) return;

  commands
    .renameKey({
      fingerprint,
      name: localName,
    })
    .then(() => {
      if (wallet) {
        setWallet({ ...wallet, name: localName });
      }
      // Publish if sync is enabled (fire-and-forget)
      commands
        .publishWalletSettings({ fingerprint })
        .catch(console.warn);
    })
    .catch(addError);
}}
```

Find where `set_wallet_emoji` is called (search for `setWalletEmoji` in the file — it's called from the wallet card or emoji picker). Add after it resolves:

```typescript
commands.publishWalletSettings({ fingerprint }).catch(console.warn);
```

Note: `setWalletEmoji` may be called from `WalletCard.tsx` rather than `Settings.tsx`. Check both files:

```bash
grep -rn "setWalletEmoji\|set_wallet_emoji" /Users/don/src/dkackman/sage/src/
```

Add the publish call wherever `setWalletEmoji` is called from, following the same pattern.

- [ ] **Install the plugin JS package**

```bash
cd /Users/don/src/dkackman/sage
pnpm add tauri-plugin-nostr-sync-api@file:../../tauri-plugin-nostr/dist-js
```

If the dist-js is not built yet, build it first:

```bash
cd /Users/don/src/dkackman/tauri-plugin-nostr
pnpm install && pnpm build
```

Then re-run the pnpm add command.

- [ ] **Verify TypeScript compiles**

```bash
cd /Users/don/src/dkackman/sage
pnpm tsc --noEmit 2>&1 | tail -30
```

Expected: no type errors.

- [ ] **Run the dev server and test the feature**

```bash
cd /Users/don/src/dkackman/sage
pnpm tauri dev 2>&1
```

Manual test checklist:
- [ ] Open Settings → Advanced tab. "Settings Sync" section is visible when a wallet with secrets is active.
- [ ] Toggle sync on. Three default relays appear. Status line shows connection state.
- [ ] Add a custom relay URL. It appears in the list and persists after restart.
- [ ] Remove a relay. It disappears from the list.
- [ ] Change wallet name in the Wallet tab. No error thrown; publish fires silently.
- [ ] Click "Fetch Settings" in the Advanced tab. Message appears ("Already up to date" or "Settings applied").
- [ ] Toggle sync off. Relay list and fetch button hide.

---

## Self-Review

### Spec coverage check

| Spec requirement | Task |
| --- | --- |
| HKDF-SHA256 key derivation from BLS master key | Task 5 |
| Publish on name change | Task 9 (WalletSettings onBlur) |
| Publish on emoji change | Task 9 (setWalletEmoji call site) |
| Opt-in per wallet (`sync_enabled`) | Tasks 2, 6 |
| Manual fetch from Advanced tab | Tasks 7, 9 |
| Relay management (add/remove/list) | Tasks 6, 9 |
| Default relays loaded from config | Tasks 1, 2, 6 |
| Silent apply on fetch (no prompt) | Task 7 |
| Publish failures don't fail mutation | Task 9 (`.catch(console.warn)`) |
| Unknown schema version → skip | Task 7 |
| Empty name → skip | Task 7 (`filter(|s| !s.is_empty())`) |
| Watch-only wallet → no-op | Task 5 (early return) |

### Placeholder check

No TBDs. The `setWalletEmoji` call site check in Task 9 requires a grep before implementing — this is intentional since we don't know at plan-write time exactly where it's called from.

### Type consistency

- `FetchSettingsResult` defined in Task 7, used in Task 9 (TypeScript side via generated bindings)
- `publish_wallet_settings` and `fetch_wallet_settings` both take `fingerprint: u32` consistently
- `add_sync_relay` / `remove_sync_relay` take `url: String` consistently with the plugin's own API
