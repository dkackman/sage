# Wallet Settings Sync — Design

**Date:** 2026-05-05  
**Branch:** sync-settings  
**Scope:** Phase 1 — wallet name and emoji, opt-in, manual fetch

---

## Overview

Sync wallet display settings (name, emoji) across app instances using `tauri-plugin-nostr-sync` as the transport. Each wallet derives its own Nostr identity from its master secret key via HKDF, so only instances holding the same wallet secret can read or write its settings. Sync is opt-in per wallet. Publish happens automatically on every mutation; reading is manually triggered from the Advanced settings tab for now.

---

## Architecture

Four layers:

1. **Plugin registration** — `tauri-plugin-nostr-sync` is added to sage's Tauri plugin chain in `lib.rs` with namespace `"sage"` and three default relays (`wss://relay.damus.io`, `wss://relay.nostr.band`, `wss://nos.lol`).

2. **Signer lifecycle** — On wallet login, sage derives a secp256k1 Nostr keypair from the wallet's BLS master secret key using HKDF-SHA256 with domain label `b"sage-nostr-sync"`. The 32-byte output becomes a `nostr_sdk::Keys` and is injected into the plugin via `app.nostr_sync().set_signer(keys)`. On logout, `clear_signer()` is called. Watch-only wallets (no secrets) skip injection — publish/fetch return `SignerNotSet`, treated as a no-op.

3. **Publish on mutation** — `rename_key` and the emoji setter, after persisting to config, check `sync_enabled` on the wallet. If true, they call `app.nostr_sync().publish("wallet-settings", payload)`. Publish failures are logged but do not fail the mutation.

4. **Manual fetch** — A new `fetch_wallet_settings` Tauri command calls the plugin's fetch for category `"wallet-settings"`, then writes the returned name and emoji directly to the `Wallet` config — bypassing the mutation commands so that a fetch does not trigger another publish.

---

## Data Model

### Wallet config (`sage-config`)

One new field added to the `Wallet` struct:

```rust
pub sync_enabled: bool,  // serde default = false
```

### App-level sync config

A new `SyncConfig` struct stored alongside the existing app config (not per-wallet, since relays are shared):

```rust
pub struct SyncConfig {
    pub relays: Vec<String>,  // default: the three relays listed above
}
```

Relays are loaded at startup and added to the plugin via `add_relay`. The plugin's runtime relay state is authoritative; `SyncConfig` is just the persisted seed list.

### Published payload

Category: `"wallet-settings"`

```json
{
  "v": 1,
  "name": "My Wallet",
  "emoji": "🦋"
}
```

- `v` is a schema version. A receiver that sees an unknown version logs a warning and skips applying.
- `emoji` may be `null` if no emoji is set.
- The NIP-33 d-tag is `sage/wallet-settings/v1`; relays retain only the latest event per wallet pubkey, so there is no history to reconcile.

### Conflict resolution

The plugin uses NIP-33 parameterized replaceable events. The relay retains only the most recent event by timestamp. Latest-write-wins is the conflict model — no merge, no prompt.

---

## New Tauri Commands

| Command | Signature | Purpose |
| --- | --- | --- |
| `get_sync_config` | `(fingerprint: u32) -> SyncConfigResponse` | Returns `{ enabled: bool }` for the given wallet |
| `set_sync_enabled` | `(fingerprint: u32, enabled: bool) -> ()` | Persists `sync_enabled` for the wallet |
| `fetch_wallet_settings` | `(fingerprint: u32) -> FetchSettingsResponse` | Fetches and applies remote settings; returns `{ applied: bool, name?: string, emoji?: string }` |

Relay management uses the plugin's existing JS bindings (`addRelay`, `removeRelay`, `getRelays`) directly — no new Tauri commands needed.

---

## Frontend — Advanced Settings Tab

A new **Settings Sync** section is added to the Advanced tab, rendered only when a wallet is logged in. It sits above the existing RpcSettings and LogViewer sections.

### Controls

**Sync toggle**  
`SettingItem` with a `Switch`. On enable: calls `set_sync_enabled(true)`. On disable: calls `set_sync_enabled(false)`. Watch-only wallets see the toggle disabled with a note: "Sync requires a wallet with secrets."

**Status line**  
Shown when sync is enabled. Displays plugin readiness: `"Sync ready (2/3 relays connected)"` or `"Not connected"`. Sourced from `getStatus()` polled on mount.

**Relay list**  
Shown when sync is enabled. Lists relays from `getRelays()` with a connected/disconnected indicator dot. Each entry has a trash button (`removeRelay(url)`). Below the list: a text input + "Add" button (`addRelay(url)`).

**Fetch button**  
`"Fetch Settings"` button calls `fetch_wallet_settings`. Shows one of:

- `"Settings applied — name and emoji updated"` on success with changes
- `"Already up to date"` if fetched values match local
- `"No remote settings found"` if no Nostr event exists yet
- Inline error message on failure

---

## Key Derivation Detail

```text
ikm  = wallet_master_secret_key_bytes   // 32-byte BLS private key
salt = []                               // empty (key is already high-entropy)
info = b"sage-nostr-sync"
okm  = HKDF-SHA256(ikm, salt, info, 32)
nostr_keys = nostr_sdk::Keys::from_secret_key(SecretKey::from_bytes(okm))
```

The derived Nostr pubkey serves as the wallet's sync identity. Two app instances holding the same wallet secret will derive the same pubkey and can therefore read each other's NIP-44-encrypted events.

---

## Error Handling

| Scenario | Behavior |
| --- | --- |
| Publish fails (no relay connection) | Log warning, mutation still succeeds |
| Publish fails (signer not set) | Silently skip — watch-only wallet |
| Fetch fails (network) | Surface error inline in Advanced tab |
| Fetch returns unknown schema version | Log warning, do not apply, return `applied: false` |
| Applied name is empty string | Skip name update, keep existing |

---

## Out of Scope (Phase 1)

- Automatic background polling or startup sync
- Syncing any settings other than wallet name and emoji
- App-level (non-wallet) settings sync
- Relay authentication (NIP-42)
- Multiple relay write strategies (currently: all connected)
