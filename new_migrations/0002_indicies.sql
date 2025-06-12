CREATE INDEX idx_transactions_height ON transactions(height);
CREATE INDEX idx_coins_asset_id ON coins(asset_id);
CREATE INDEX idx_coins_created_height ON coins(created_height);
CREATE INDEX idx_nft_data_nft_id ON nft_data(nft_id);
CREATE INDEX idx_nfts_collection_id ON nfts(collection_id);
