use std::net::IpAddr;

use chia::protocol::Bytes32;
use sage_database::OfferStatus;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncEvent {
    Start(IpAddr),
    Stop,
    Subscribed,
    DerivationIndex {
        next_index: u32,
    },
    CoinsUpdated,
    TransactionUpdated {
        transaction_id: Bytes32,
    },
    TransactionEnded {
        transaction_id: Bytes32,
        error: Option<String>,
        success: bool,
    },
    OfferUpdated {
        offer_id: Bytes32,
        status: OfferStatus,
    },
    PuzzleBatchSynced,
    CatInfo,
    DidInfo,
    NftData,
}
