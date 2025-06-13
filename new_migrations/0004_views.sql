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
	LEFT JOIN blocks AS spent_blocks ON spent_blocks.height = transactions.height