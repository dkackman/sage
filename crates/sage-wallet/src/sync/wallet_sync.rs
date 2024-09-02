use std::{sync::Arc, time::Duration};

use chia::{
    bls::{DerivableKey, PublicKey},
    protocol::{Bytes32, CoinStateFilters, RejectStateReason},
    puzzles::{standard::StandardArgs, DeriveSynthetic},
};
use chia_wallet_sdk::Peer;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use sage_database::Database;
use tokio::{
    sync::{mpsc, Mutex},
    task::spawn_blocking,
    time::timeout,
};
use tracing::{debug, info, instrument, warn};

use crate::{SyncError, WalletError};

use super::{PeerState, SyncEvent};

pub async fn sync_wallet(
    db: Database,
    intermediate_pk: PublicKey,
    genesis_challenge: Bytes32,
    peer: Peer,
    state: Arc<Mutex<PeerState>>,
    sync_sender: mpsc::Sender<SyncEvent>,
) -> Result<(), WalletError> {
    info!("Starting sync against peer {}", peer.socket_addr());

    let mut tx = db.tx().await?;
    let p2_puzzle_hashes = tx.p2_puzzle_hashes().await?;
    let (start_height, start_header_hash) = tx.latest_peak().await?.map_or_else(
        || (None, genesis_challenge),
        |(peak, header_hash)| (Some(peak), header_hash),
    );
    tx.commit().await?;

    let mut derive_more = p2_puzzle_hashes.is_empty();

    for batch in p2_puzzle_hashes.chunks(1000) {
        derive_more |= sync_puzzle_hashes(
            &db,
            &peer,
            start_height,
            start_header_hash,
            batch,
            sync_sender.clone(),
        )
        .await?;
    }

    let mut start_index = p2_puzzle_hashes.len() as u32;

    while derive_more {
        derive_more = false;

        let new_derivations = spawn_blocking(move || {
            (start_index..start_index + 1000)
                .into_par_iter()
                .map(|index| {
                    let synthetic_key = intermediate_pk.derive_unhardened(index).derive_synthetic();
                    let p2_puzzle_hash =
                        Bytes32::from(StandardArgs::curry_tree_hash(synthetic_key));
                    (index, synthetic_key, p2_puzzle_hash)
                })
                .collect::<Vec<_>>()
        })
        .await?;

        let p2_puzzle_hashes: Vec<Bytes32> = new_derivations
            .iter()
            .map(|(_, _, p2_puzzle_hash)| *p2_puzzle_hash)
            .collect();

        start_index += new_derivations.len() as u32;

        let mut tx = db.tx().await?;
        for (index, synthetic_key, p2_puzzle_hash) in new_derivations {
            tx.insert_derivation(p2_puzzle_hash, index, false, synthetic_key)
                .await?;
        }
        tx.commit().await?;

        for batch in p2_puzzle_hashes.chunks(1000) {
            derive_more |= sync_puzzle_hashes(
                &db,
                &peer,
                None,
                genesis_challenge,
                batch,
                sync_sender.clone(),
            )
            .await?;
        }
    }

    if let Some((height, header_hash)) = state.lock().await.peak() {
        // TODO: Maybe look into a better way.
        info!(
            "Updating peak to {} with header hash {}",
            height, header_hash
        );
        db.insert_peak(height, header_hash).await?;
    } else {
        warn!("No peak found");
    }

    Ok(())
}

#[instrument(skip(db, peer, puzzle_hashes))]
async fn sync_puzzle_hashes(
    db: &Database,
    peer: &Peer,
    start_height: Option<u32>,
    start_header_hash: Bytes32,
    puzzle_hashes: &[Bytes32],
    sync_sender: mpsc::Sender<SyncEvent>,
) -> Result<bool, WalletError> {
    let mut prev_height = start_height;
    let mut prev_header_hash = start_header_hash;
    let mut found_coins = false;

    loop {
        debug!(
            "Requesting coins at height {:?} and header hash {} from peer {}",
            prev_height,
            prev_header_hash,
            peer.socket_addr()
        );

        let response = timeout(
            Duration::from_secs(45),
            peer.request_puzzle_state(
                puzzle_hashes.to_vec(),
                prev_height,
                prev_header_hash,
                CoinStateFilters::new(true, true, true, 0),
                true,
            ),
        )
        .await
        .map_err(|_| WalletError::Sync(SyncError::Timeout))??;

        match response {
            Ok(data) => {
                debug!("Received {} coin states", data.coin_states.len());

                let mut tx = db.tx().await?;

                for coin_state in data.coin_states {
                    found_coins = true;

                    let is_p2 = tx.is_p2_puzzle_hash(coin_state.coin.puzzle_hash).await?;

                    tx.insert_coin_state(coin_state, is_p2).await?;

                    if is_p2 {
                        tx.insert_p2_coin(coin_state.coin.coin_id()).await?;
                    }
                }

                tx.commit().await?;

                sync_sender.send(SyncEvent::CoinUpdate).await.ok();

                prev_height = Some(data.height);
                prev_header_hash = data.header_hash;

                if data.is_finished {
                    break;
                }
            }
            Err(rejection) => match rejection.reason {
                RejectStateReason::ExceededSubscriptionLimit => {
                    warn!(
                        "Subscription limit reached against peer {}",
                        peer.socket_addr()
                    );
                    return Err(WalletError::Sync(SyncError::SubscriptionLimit));
                }
                RejectStateReason::Reorg => {
                    // TODO: Handle reorgs gracefully
                    todo!()
                }
            },
        }
    }

    Ok(found_coins)
}
