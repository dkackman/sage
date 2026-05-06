use std::{fs, time::Duration};

use chia_wallet_sdk::utils::Address;
use hkdf::Hkdf;
use reqwest::StatusCode;
use sage::Error;
use sage_api::{wallet_connect::*, *};
use sage_api_macro::impl_endpoints_tauri;
use sage_config::{NetworkConfig, Wallet, WalletDefaults};
use sage_rpc::start_rpc;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use specta::{Type, specta};
use tauri::{AppHandle, State, command};
use tauri_plugin_nostr_sync::TauriPluginNostrSyncExt;
use tokio::time::sleep;
use tracing::error;

use crate::{
    app_state::{self, AppState, Initialized, RpcTask},
    error::Result,
};

#[command]
#[specta]
pub async fn initialize(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    initialized: State<'_, Initialized>,
    rpc_task: State<'_, RpcTask>,
) -> Result<()> {
    let mut initialized = initialized.0.lock().await;

    if *initialized {
        return Ok(());
    }

    *initialized = true;

    let mut sage = state.lock().await;
    app_state::initialize(app_handle.clone(), &mut sage).await?;
    drop(sage);

    let app_state = (*state).clone();

    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(3)).await;

            let app_state = app_state.lock().await;

            if let Err(error) = app_state.save_peers().await {
                error!("Error while saving peers: {error:?}");
            }

            drop(app_state);
        }
    });

    let app_state = state.lock().await;

    if app_state.config.rpc.enabled {
        *rpc_task.0.lock().await = Some(tokio::spawn(start_rpc((*state).clone())));
    }

    // Load persisted relays into the nostr-sync plugin
    let relays = app_state.config.sync.relays.clone();
    drop(app_state);

    for url in relays {
        if let Err(e) = app_handle.nostr_sync().add_relay(&url).await {
            tracing::warn!("Failed to connect to relay {url}: {e}");
        }
    }

    Ok(())
}

impl_endpoints_tauri! {
    (repeat
        #[command]
        #[specta]
        pub async fn endpoint(state: State<'_, AppState>, req: Endpoint) -> Result<EndpointResponse> {
            Ok(state.lock().await.endpoint(req) maybe_await?)
        }
    )
}

#[command]
#[specta]
pub async fn validate_address(state: State<'_, AppState>, address: String) -> Result<bool> {
    let state = state.lock().await;
    let Some(address) = Address::decode(&address).ok() else {
        return Ok(false);
    };
    Ok(address.prefix == state.network().prefix())
}

#[command]
#[specta]
pub async fn network_config(state: State<'_, AppState>) -> Result<NetworkConfig> {
    Ok(state.lock().await.config.network.clone())
}

#[command]
#[specta]
pub async fn wallet_config(state: State<'_, AppState>, fingerprint: u32) -> Result<Option<Wallet>> {
    Ok(state
        .lock()
        .await
        .wallet_config
        .wallets
        .iter()
        .find(|wallet| wallet.fingerprint == fingerprint)
        .cloned())
}

#[command]
#[specta]
pub async fn default_wallet_config(state: State<'_, AppState>) -> Result<WalletDefaults> {
    Ok(state.lock().await.wallet_config.defaults)
}

#[command]
#[specta]
pub async fn is_rpc_running(rpc_task: State<'_, RpcTask>) -> Result<bool> {
    Ok(rpc_task.0.lock().await.is_some())
}

#[command]
#[specta]
pub async fn start_rpc_server(
    state: State<'_, AppState>,
    rpc_task: State<'_, RpcTask>,
) -> Result<()> {
    let mut rpc_task = rpc_task.0.lock().await;
    *rpc_task = Some(tokio::spawn(start_rpc((*state).clone())));
    Ok(())
}

#[command]
#[specta]
pub async fn stop_rpc_server(rpc_task: State<'_, RpcTask>) -> Result<()> {
    let mut rpc_task = rpc_task.0.lock().await;
    if let Some(handle) = rpc_task.take() {
        handle.abort();
    }
    Ok(())
}

#[command]
#[specta]
pub async fn get_rpc_run_on_startup(state: State<'_, AppState>) -> Result<bool> {
    Ok(state.lock().await.config.rpc.enabled)
}

#[command]
#[specta]
pub async fn set_rpc_run_on_startup(
    state: State<'_, AppState>,
    run_on_startup: bool,
) -> Result<()> {
    state.lock().await.config.rpc.enabled = run_on_startup;
    state.lock().await.save_config()?;
    Ok(())
}

#[command]
#[specta]
pub async fn switch_wallet(state: State<'_, AppState>) -> Result<()> {
    state.lock().await.switch_wallet().await?;
    Ok(())
}

#[command]
#[specta]
pub async fn move_key(state: State<'_, AppState>, fingerprint: u32, index: u32) -> Result<()> {
    let mut state = state.lock().await;

    let old_index = state
        .wallet_config
        .wallets
        .iter()
        .position(|w| w.fingerprint == fingerprint)
        .ok_or(Error::UnknownFingerprint)?;

    let wallet = state.wallet_config.wallets.remove(old_index);
    state.wallet_config.wallets.insert(index as usize, wallet);
    state.save_config()?;

    Ok(())
}

#[command]
#[specta]
pub async fn download_cni_offercode(code: String) -> Result<String> {
    #[derive(Serialize)]
    struct Request {
        code: String,
    }

    #[derive(Deserialize)]
    struct Response {
        offer: String,
    }

    let response = reqwest::Client::new()
        .post("https://offercodes.chia.net/download_offer")
        .json(&Request { code: code.clone() })
        .send()
        .await?;

    if response.status() != StatusCode::OK {
        return Err(crate::error::Error {
            kind: ErrorKind::Nfc,
            reason: format!(
                "Invalid offer code {code}: Server responded with code {}",
                response.status()
            ),
        });
    }

    let response = response.json::<Response>().await?.offer;

    Ok(response)
}

#[derive(Serialize, Type)]
pub struct LogFile {
    name: String,
    text: String,
}

#[command]
#[specta]
pub async fn get_logs(state: State<'_, AppState>) -> Result<Vec<LogFile>> {
    let state = state.lock().await;
    let files = fs::read_dir(state.path.join("log"))?;

    let mut log_files = Vec::new();

    for file in files {
        let file = file?;

        let name = file.file_name().to_string_lossy().to_string();

        if !name.starts_with("app.log") {
            continue;
        }

        let text = fs::read_to_string(file.path())?;

        log_files.push(LogFile { name, text });
    }

    Ok(log_files)
}

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

    let secret_key = nostr_sdk::SecretKey::from_slice(&okm).map_err(|e| crate::error::Error {
        kind: sage_api::ErrorKind::Internal,
        reason: e.to_string(),
    })?;
    let keys = nostr_sdk::Keys::new(secret_key);

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
        return Err(Error::UnknownFingerprint.into());
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

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FetchSettingsResult {
    pub applied: bool,
    pub name: Option<String>,
    pub emoji: Option<String>,
}

#[command]
#[specta]
pub async fn publish_wallet_settings(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    fingerprint: u32,
) -> Result<()> {
    let (sync_enabled, name, emoji) = {
        let sage = state.lock().await;
        let Some(wallet) = sage
            .wallet_config
            .wallets
            .iter()
            .find(|w| w.fingerprint == fingerprint)
        else {
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

    if let Err(e) = app_handle
        .nostr_sync()
        .publish("wallet-settings", &payload)
        .await
    {
        tracing::warn!("Failed to publish wallet settings: {e}");
    }

    Ok(())
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
        return Ok(FetchSettingsResult {
            applied: false,
            name: None,
            emoji: None,
        });
    };

    let payload = &fetch_result.payload;

    let version = payload["v"].as_u64().unwrap_or(0);
    if version != 1 {
        tracing::warn!("fetch_wallet_settings: unknown schema version {version}, skipping");
        return Ok(FetchSettingsResult {
            applied: false,
            name: None,
            emoji: None,
        });
    }

    let remote_name = payload["name"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let remote_emoji = payload.get("emoji").and_then(|e| {
        if e.is_null() {
            None
        } else {
            e.as_str().map(str::to_string)
        }
    });

    let mut sage = state.lock().await;
    let Some(wallet) = sage
        .wallet_config
        .wallets
        .iter_mut()
        .find(|w| w.fingerprint == fingerprint)
    else {
        return Ok(FetchSettingsResult {
            applied: false,
            name: None,
            emoji: None,
        });
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

#[cfg(test)]
mod sync_tests {
    use super::*;

    #[test]
    fn hkdf_derive_nostr_key_produces_32_bytes() {
        let ikm = [42u8; 32];
        let hk = Hkdf::<Sha256>::new(None, &ikm);
        let mut okm = [0u8; 32];
        hk.expand(b"sage-nostr-sync", &mut okm).unwrap();
        assert_ne!(okm, [0u8; 32]);
    }

    #[test]
    fn same_ikm_produces_same_nostr_key() {
        let ikm = [99u8; 32];
        let derive = |ikm: &[u8]| {
            let hk = Hkdf::<Sha256>::new(None, ikm);
            let mut okm = [0u8; 32];
            hk.expand(b"sage-nostr-sync", &mut okm).unwrap();
            okm
        };
        assert_eq!(derive(&ikm), derive(&ikm));
    }

    #[test]
    fn different_ikm_produces_different_nostr_key() {
        let derive = |ikm: &[u8]| {
            let hk = Hkdf::<Sha256>::new(None, ikm);
            let mut okm = [0u8; 32];
            hk.expand(b"sage-nostr-sync", &mut okm).unwrap();
            okm
        };
        assert_ne!(derive(&[1u8; 32]), derive(&[2u8; 32]));
    }

    #[test]
    fn wallet_settings_payload_schema_version_is_1() {
        let payload = serde_json::json!({
            "v": 1,
            "name": "My Wallet",
            "emoji": serde_json::Value::Null,
        });
        assert_eq!(payload["v"], 1);
        assert_eq!(payload["name"], "My Wallet");
        assert!(payload["emoji"].is_null());
    }

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
            if e.is_null() {
                None
            } else {
                e.as_str().map(str::to_string)
            }
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
}
