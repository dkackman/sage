use std::collections::HashMap;

use chia::{
    bls::{
        master_to_wallet_hardened_intermediate, master_to_wallet_unhardened_intermediate, sign,
        DerivableKey, PublicKey, SecretKey,
    },
    protocol::{Bytes32, SpendBundle},
    puzzles::DeriveSynthetic,
};
use chia_wallet_sdk::signer::{AggSigConstants, RequiredSignature};
use clvmr::Allocator;
use itertools::Itertools;

use crate::WalletError;

use super::Wallet;

impl Wallet {
    pub async fn sign_transaction(
        &self,
        spend_bundle: SpendBundle,
        agg_sig_constants: &AggSigConstants,
        master_sk: SecretKey,
        partial: bool,
    ) -> Result<SpendBundle, WalletError> {
        let input_coin_spends = spend_bundle
            .coin_spends
            .iter()
            .filter(|cs| cs.coin.parent_coin_info != Bytes32::default())
            .cloned()
            .collect_vec();

        let required_signatures = RequiredSignature::from_coin_spends(
            &mut Allocator::new(),
            &input_coin_spends,
            agg_sig_constants,
        )?;

        let mut derivations = HashMap::new();

        for required in &required_signatures {
            let RequiredSignature::Bls(required) = required else {
                continue;
            };
            let Some(derivation) = self.db.derivation(required.public_key).await? else {
                if partial {
                    continue;
                }
                return Err(WalletError::UnknownPublicKey);
            };
            derivations.insert(required.public_key, derivation);
        }

        let unhardened_intermediate_sk = master_to_wallet_unhardened_intermediate(&master_sk);
        let hardened_intermediate_sk = master_to_wallet_hardened_intermediate(&master_sk);

        let secret_keys: HashMap<PublicKey, SecretKey> = derivations
            .iter()
            .map(|(public_key, derivation)| {
                let secret_key = if derivation.is_hardened {
                    hardened_intermediate_sk.derive_hardened(derivation.derivation_index)
                } else {
                    unhardened_intermediate_sk.derive_unhardened(derivation.derivation_index)
                }
                .derive_synthetic();

                (*public_key, secret_key)
            })
            .collect();

        let mut aggregated_signature = spend_bundle.aggregated_signature;

        for required in required_signatures {
            let RequiredSignature::Bls(required) = required else {
                continue;
            };
            let Some(sk) = secret_keys.get(&required.public_key).cloned() else {
                continue;
            };
            aggregated_signature += &sign(&sk, required.message());
        }

        Ok(SpendBundle::new(
            spend_bundle.coin_spends,
            aggregated_signature,
        ))
    }
}
