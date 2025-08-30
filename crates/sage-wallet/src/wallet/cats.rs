use chia::{
    bls::PublicKey,
    clvm_utils::ToTreeHash,
    protocol::{Bytes, Bytes32, CoinSpend},
    puzzles::cat::EverythingWithSignatureTailArgs,
};
use chia_wallet_sdk::driver::{Action, ClawbackV2, Id, Spend, SpendContext};
use clvmr::NodePtr;

use crate::{wallet::memos::Hint, WalletError};

use super::{memos::calculate_memos, Wallet};

impl Wallet {
    pub async fn issue_cat(
        &self,
        amount: u64,
        fee: u64,
        multi_issuance_key: Option<PublicKey>,
    ) -> Result<(Vec<CoinSpend>, Bytes32), WalletError> {
        let mut ctx = SpendContext::new();

        let issue_cat = if let Some(public_key) = multi_issuance_key {
            let tail = ctx.curry(EverythingWithSignatureTailArgs::new(public_key))?;
            let tail_spend = Spend::new(tail, NodePtr::NIL);
            Action::issue_cat(tail_spend, None, amount)
        } else {
            Action::single_issue_cat(None, amount)
        };
        let actions = vec![Action::fee(fee), issue_cat];
        let outputs = self.spend(&mut ctx, vec![], &actions).await?;

        Ok((ctx.take(), outputs.cats[&Id::New(1)][0].info.asset_id))
    }

    /// Sends the given amount of CAT to the given puzzle hash.
    pub async fn send_cat(
        &self,
        asset_id: Bytes32,
        amounts: Vec<(Bytes32, u64)>,
        fee: u64,
        include_hint: bool,
        memos: Vec<Bytes>,
        clawback: Option<u64>,
    ) -> Result<Vec<CoinSpend>, WalletError> {
        let sender_puzzle_hash = self.change_p2_puzzle_hash().await?;

        let mut ctx = SpendContext::new();
        let mut actions = vec![Action::fee(fee)];

        for (puzzle_hash, amount) in amounts {
            let clawback = clawback.map(|seconds| {
                ClawbackV2::new(sender_puzzle_hash, puzzle_hash, seconds, amount, true)
            });

            let memos = calculate_memos(
                &mut ctx,
                if let Some(clawback) = clawback {
                    Hint::Clawback(clawback)
                } else if include_hint {
                    Hint::P2PuzzleHash(puzzle_hash)
                } else {
                    Hint::None
                },
                memos.clone(),
            )?;

            let p2_puzzle_hash = if let Some(clawback) = clawback {
                clawback.tree_hash().into()
            } else {
                puzzle_hash
            };

            actions.push(Action::send(
                Id::Existing(asset_id),
                p2_puzzle_hash,
                amount,
                memos,
            ));
        }

        self.spend(&mut ctx, vec![], &actions).await?;

        Ok(ctx.take())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use test_log::test;
    use tokio::time::sleep;

    use crate::TestWallet;

    #[test(tokio::test)]
    async fn test_send_cat() -> anyhow::Result<()> {
        let mut test = TestWallet::new(1500).await?;

        let (coin_spends, asset_id) = test.wallet.issue_cat(1000, 0, None).await?;
        assert_eq!(coin_spends.len(), 2);

        test.transact(coin_spends).await?;
        test.wait_for_coins().await;

        assert_eq!(test.wallet.db.xch_balance().await?, 500);
        assert_eq!(test.wallet.db.selectable_xch_coins().await?.len(), 1);
        assert_eq!(test.wallet.db.cat_balance(asset_id).await?, 1000);
        assert_eq!(
            test.wallet.db.selectable_cat_coins(asset_id).await?.len(),
            1
        );

        let coin_spends = test
            .wallet
            .send_cat(
                asset_id,
                vec![(test.puzzle_hash, 750)],
                0,
                true,
                vec![],
                None,
            )
            .await?;
        assert_eq!(coin_spends.len(), 1);

        test.transact(coin_spends).await?;
        test.wait_for_coins().await;

        assert_eq!(test.wallet.db.cat_balance(asset_id).await?, 1000);
        assert_eq!(
            test.wallet.db.selectable_cat_coins(asset_id).await?.len(),
            2
        );

        let coin_spends = test
            .wallet
            .send_cat(
                asset_id,
                vec![(test.puzzle_hash, 1000)],
                500,
                true,
                vec![],
                None,
            )
            .await?;
        assert_eq!(coin_spends.len(), 3);

        test.transact(coin_spends).await?;
        test.wait_for_coins().await;

        assert_eq!(test.wallet.db.xch_balance().await?, 0);
        assert_eq!(test.wallet.db.selectable_xch_coins().await?.len(), 0);
        assert_eq!(test.wallet.db.cat_balance(asset_id).await?, 1000);
        assert_eq!(
            test.wallet.db.selectable_cat_coins(asset_id).await?.len(),
            1
        );

        Ok(())
    }

    #[test(tokio::test)]
    async fn test_send_cat_with_clawback_self() -> anyhow::Result<()> {
        let mut test = TestWallet::new(1000).await?;

        let (coin_spends, asset_id) = test.wallet.issue_cat(1000, 0, None).await?;
        assert_eq!(coin_spends.len(), 2);

        test.transact(coin_spends).await?;
        test.wait_for_coins().await;

        let timestamp = test.new_block_with_current_time().await?;

        let coin_spends = test
            .wallet
            .send_cat(
                asset_id,
                vec![(test.puzzle_hash, 1000)],
                0,
                true,
                vec![],
                Some(timestamp + 5),
            )
            .await?;

        assert_eq!(coin_spends.len(), 1);

        test.transact(coin_spends).await?;
        test.wait_for_coins().await;

        assert_eq!(test.wallet.db.selectable_cat_balance(asset_id).await?, 0);
        assert_eq!(
            test.wallet.db.selectable_cat_coins(asset_id).await?.len(),
            0
        );

        sleep(Duration::from_secs(6)).await;
        test.new_block_with_current_time().await?;

        assert_eq!(test.wallet.db.selectable_cat_balance(asset_id).await?, 1000);
        assert_eq!(
            test.wallet.db.selectable_cat_coins(asset_id).await?.len(),
            1
        );

        let coin_spends = test
            .wallet
            .send_cat(
                asset_id,
                vec![(test.puzzle_hash, 1000)],
                0,
                true,
                vec![],
                None,
            )
            .await?;

        assert_eq!(coin_spends.len(), 1);

        test.transact(coin_spends).await?;
        test.wait_for_coins().await;

        assert_eq!(test.wallet.db.cat_balance(asset_id).await?, 1000);
        assert_eq!(
            test.wallet.db.selectable_cat_coins(asset_id).await?.len(),
            1
        );

        Ok(())
    }

    #[test(tokio::test)]
    async fn test_send_cat_with_clawback_external() -> anyhow::Result<()> {
        let mut alice = TestWallet::new(1000).await?;
        let mut bob = alice.next(0).await?;

        let (coin_spends, asset_id) = alice.wallet.issue_cat(1000, 0, None).await?;
        assert_eq!(coin_spends.len(), 2);

        alice.transact(coin_spends).await?;
        alice.wait_for_coins().await;

        let timestamp = alice.new_block_with_current_time().await?;

        let coin_spends = alice
            .wallet
            .send_cat(
                asset_id,
                vec![(bob.puzzle_hash, 1000)],
                0,
                true,
                vec![],
                Some(timestamp + 5),
            )
            .await?;

        assert_eq!(coin_spends.len(), 1);

        alice.transact(coin_spends).await?;

        alice.wait_for_coins().await;

        assert_eq!(alice.wallet.db.selectable_cat_balance(asset_id).await?, 0);
        assert_eq!(
            alice.wallet.db.selectable_cat_coins(asset_id).await?.len(),
            0
        );

        bob.wait_for_puzzles().await;

        assert_eq!(bob.wallet.db.selectable_cat_balance(asset_id).await?, 0);
        assert_eq!(bob.wallet.db.selectable_cat_coins(asset_id).await?.len(), 0);

        sleep(Duration::from_secs(6)).await;
        bob.new_block_with_current_time().await?;

        assert_eq!(alice.wallet.db.selectable_cat_balance(asset_id).await?, 0);
        assert_eq!(
            alice.wallet.db.selectable_cat_coins(asset_id).await?.len(),
            0
        );

        assert_eq!(bob.wallet.db.selectable_cat_balance(asset_id).await?, 1000);
        assert_eq!(bob.wallet.db.selectable_cat_coins(asset_id).await?.len(), 1);

        let coin_spends = bob
            .wallet
            .send_cat(
                asset_id,
                vec![(alice.puzzle_hash, 1000)],
                0,
                true,
                vec![],
                None,
            )
            .await?;

        assert_eq!(coin_spends.len(), 1);

        bob.transact(coin_spends).await?;
        bob.wait_for_coins().await;
        alice.wait_for_puzzles().await;

        assert_eq!(
            alice.wallet.db.selectable_cat_balance(asset_id).await?,
            1000
        );
        assert_eq!(
            alice.wallet.db.selectable_cat_coins(asset_id).await?.len(),
            1
        );

        assert_eq!(bob.wallet.db.selectable_cat_balance(asset_id).await?, 0);
        assert_eq!(bob.wallet.db.selectable_cat_coins(asset_id).await?.len(), 0);

        Ok(())
    }
}
