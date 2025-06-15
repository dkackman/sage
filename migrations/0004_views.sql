/*
	These views are for transition purposes. They overlay the old schema on 
	top of the new schema. They need to be replaced once the new schema is fully
	vetted and Rust code rewritten.
*/

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
	AND tokens.id > 0; -- xch is not a cat coins

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

CREATE VIEW cats AS
SELECT
	assets.hash AS asset_id,
	assets.name,
	tokens.ticker,
	assets.is_visible AS visible,
	assets.icon_url AS icon,
	assets.description,
	assets.is_pending AS fetched,
	TRUE AS is_named	
FROM assets
	INNER JOIN tokens ON tokens.asset_id = assets.id
WHERE 1=1
	AND assets.kind = 0
	AND tokens.id > 0; -- xch is not a cat coins

CREATE VIEW dids_ AS
SELECT
	assets.hash AS launcher_id,
	coins.hash AS coin_id,	
	assets.name,
	assets.is_visible AS visible,
	dids.is_owned,
	TRUE as is_named,
	assets.created_height,
	assets.is_pending
FROM assets
	INNER JOIN dids ON dids.asset_id = assets.id
	INNER JOIN coins ON coins.asset_id = assets.id
WHERE 1=1
	AND assets.kind = 2;

CREATE VIEW nfts_ AS
SELECT
	assets.hash AS launcher_id,
	coins.hash AS coin_id,	
	collections.hash AS collection_id,
	nfts.minter_did,
	nfts.owner_did,
	assets.is_visible AS visible,
	nfts.is_sensitive_content AS sensitive_content,
	assets.name,
	nfts.is_owned,
	TRUE as is_named,
	assets.created_height,
	assets.is_pending,
	nfts.metadata_hash,
	nfts.edition_number,
	nfts.edition_total
FROM assets
	INNER JOIN nfts ON nfts.asset_id = assets.id
	INNER JOIN coins ON coins.asset_id = assets.id
	LEFT JOIN collections ON collections.id = nfts.collection_id
WHERE 1=1
	AND assets.kind = 1;

CREATE VIEW nft_data_ AS
SELECT
	nft_data.is_hash_matched AS hash_matches,
	nft_data.mime_type,
	nft_data.data,
	nfts.data_hash AS hash
FROM nft_data
	INNER JOIN nfts ON nfts.id = nft_data.nft_id
WHERE 1=1
	AND kind = 0;

CREATE VIEW nft_thumbnails AS
SELECT
	CASE WHEN kind = 1 THEN nft_data.data ELSE NULL END AS icon,
	CASE WHEN kind = 2 THEN nft_data.data ELSE NULL END AS thumbnail,
	nfts.data_hash AS hash
FROM nft_data
	INNER JOIN nfts ON nfts.id = nft_data.nft_id
WHERE 1=1
	AND kind in (1, 2);

CREATE VIEW nft_uris AS
SELECT
	nft_data.is_hash_matched AS hash_matches,
	CAST(nft_data.data  AS TEXT) AS uri,
	nfts.data_hash AS hash,
	TRUE AS checked
FROM nft_data
	INNER JOIN nfts ON nfts.id = nft_data.nft_id
WHERE 1=1
	AND kind = 0;