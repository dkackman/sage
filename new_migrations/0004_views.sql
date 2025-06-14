CREATE VIEW coin_states AS
SELECT coins.hash AS coin_id,
	parent_coin_id,
	coins.puzzle_hash,
	amount,
	transactions.height AS spent_height,
	coins.created_height,
	hint,
    kind,
	is_synced AS synced,
	transactions.hash AS transaction_id,
	created_blocks.timestamp AS created_unixtime, 
	spent_blocks.timestamp AS spent_unixtime
  FROM coins
	LEFT JOIN transaction_coins ON transaction_coins.coin_id = coins.id
	INNER JOIN transactions ON transactions.id = transaction_coins.coin_id
	LEFT JOIN blocks AS created_blocks ON created_blocks.height = coins.created_height
	LEFT JOIN blocks AS spent_blocks ON spent_blocks.height = transactions.height;

CREATE VIEW transaction_spends AS
SELECT coins.hash AS coin_id,
	transactions.hash AS transaction_id,
	`index`,
	transaction_coins.puzzle_hash,
	coins.parent_coin_id,
	coins.amount,
	transaction_coins.puzzle_reveal,
	transaction_coins.solution
FROM transaction_coins
	INNER JOIN coins ON coins.id = transaction_coins.coin_id
	INNER JOIN transactions ON transactions.id = transaction_coins.coin_id
WHERE 1=1
	AND transaction_coins.is_spend = 1;

CREATE VIEW offered_coins AS
SELECT offers.hash AS offer_id, coins.hash AS coin_id
FROM offer_coins
	INNER JOIN coins ON coins.id = offer_coins.coin_id
	INNER JOIN offers ON offers.id = offer_coins.offer_id;

CREATE VIEW cat_coins AS
SELECT
	coins.hash AS coin_id,
	parent_parent_coin_id,
	parent_inner_puzzle_hash,
	parent_amount,
	assets.hash AS asset_id
FROM assets
	INNER JOIN coins ON coins.asset_id = assets.id
	INNER JOIN tokens ON tokens.asset_id = assets.id
	INNER JOIN lineage_proofs ON lineage_proofs.coin_id = coins.id
WHERE 1=1
	AND assets.kind = 0
	AND tokens.id > 0;

CREATE VIEW nft_coins AS
SELECT
	coins.hash AS coin_id,
	parent_parent_coin_id,
	parent_inner_puzzle_hash,
	parent_amount,
	assets.hash AS launcher_id,
	nfts.metadata,
	nfts.metadata_updater_puzzle_hash,
	nfts.current_owner,
	nfts.royalty_puzzle_hash,
	nfts.royalty_ten_thousandths,
	coins.puzzle_hash,
	nfts.data_hash,
	nfts.metadata_hash,
	nfts.license_hash
FROM assets
	INNER JOIN coins ON coins.asset_id = assets.id
	INNER JOIN nfts ON nfts.asset_id = assets.id
	INNER JOIN lineage_proofs ON lineage_proofs.coin_id = coins.id
WHERE 1=1
	AND assets.kind = 1;

CREATE VIEW did_coins AS
SELECT
	coins.hash AS coin_id,
	parent_parent_coin_id,
	parent_inner_puzzle_hash,
	parent_amount,
	assets.hash AS launcher_id,
	dids.recovery_list_hash,
	coins.puzzle_hash,
	dids.num_verifications_required,
	dids.metadata
FROM assets
	INNER JOIN coins ON coins.asset_id = assets.id
	INNER JOIN dids ON dids.asset_id = assets.id
	INNER JOIN lineage_proofs ON lineage_proofs.coin_id = coins.id
WHERE 1=1
	AND assets.kind = 2;
