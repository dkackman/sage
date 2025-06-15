CREATE TABLE derivations (
  id INTEGER,
  p2_puzzle_hash BLOB NOT NULL UNIQUE,
  `index` INTEGER NOT NULL,
  hardened BOOLEAN NOT NULL,
  synthetic_key BLOB NOT NULL,
  PRIMARY KEY (id)
);

CREATE TABLE rust_migrations (
  version INTEGER,
  PRIMARY KEY (version)
);

CREATE TABLE offers (
  id INTEGER,
  hash BLOB NOT NULL UNIQUE,
  encoded_offer TEXT NOT NULL,
  fee BLOB NOT NULL,
  status INTEGER NOT NULL,
  expiration_height INTEGER,
  expiration_timestamp INTEGER,
  inserted_timestamp INTEGER NOT NULL,
  PRIMARY KEY (id)
);

CREATE TABLE assets (
  id INTEGER,
  hash BLOB NOT NULL UNIQUE,
  kind INTEGER NOT NULL,
  name TEXT,
  icon_url TEXT,
  description TEXT,
  is_visible BOOLEAN NOT NULL,
  is_pending BOOLEAN NOT NULL,
  created_height INTEGER,
  PRIMARY KEY (id)
);

CREATE TABLE offer_assets (
  id INTEGER,
  offer_id INTEGER NOT NULL,
  asset_id INTEGER NOT NULL,
  amount BLOB NOT NULL,
  is_requested BOOLEAN NOT NULL,
  PRIMARY KEY (id),
  FOREIGN KEY (offer_id) REFERENCES offers(id) ON DELETE CASCADE,
  FOREIGN KEY (asset_id) REFERENCES assets(id) ON DELETE CASCADE,
  UNIQUE(offer_id, asset_id)
);

CREATE TABLE blocks (
  height INTEGER,
  is_peak BOOLEAN NOT NULL DEFAULT FALSE,
  header_hash BLOB,
  timestamp INTEGER,
  PRIMARY KEY (height)
);

CREATE TABLE coins (
  id INTEGER,
  asset_id INTEGER NOT NULL,
  hash BLOB NOT NULL UNIQUE,
  parent_coin_id BLOB NOT NULL,
  puzzle_hash BLOB NOT NULL,
  amount BLOB NOT NULL,
  is_synced BOOLEAN NOT NULL,
  hint BLOB,
  created_height INTEGER,
  PRIMARY KEY (id),
  FOREIGN KEY (asset_id) REFERENCES assets(id) ON DELETE CASCADE,
  FOREIGN KEY (created_height) REFERENCES blocks(height) ON DELETE SET NULL
);

CREATE TABLE lineage_proofs (
  id INTEGER,
  coin_id INTEGER NOT NULL UNIQUE,
  parent_parent_coin_id BLOB NOT NULL,
  parent_inner_puzzle_hash BLOB NOT NULL,
  parent_amount BLOB NOT NULL,
  PRIMARY KEY (id)
  FOREIGN KEY (coin_id) REFERENCES coins(id) ON DELETE CASCADE
);

CREATE TABLE transactions (
  id INTEGER,
  hash BLOB NOT NULL UNIQUE,
  aggregated_signature BLOB,
  fee BLOB,
  height INTEGER,
  submitted_at_timestamp INTEGER,
  PRIMARY KEY (id),
  FOREIGN KEY (height) REFERENCES blocks(height) ON DELETE SET NULL
);

CREATE TABLE transaction_coins (
  id INTEGER,
  transaction_id INTEGER NOT NULL,
  coin_id INTEGER NOT NULL UNIQUE,
  'index' INTEGER NOT NULL,
  puzzle_hash BLOB NOT NULL,
  puzzle_reveal BLOB NOT NULL,
  solution BLOB NOT NULL,
  is_spend BOOLEAN NOT NULL,
  PRIMARY KEY (id),
  FOREIGN KEY (transaction_id) REFERENCES transactions(id) ON DELETE CASCADE,
  UNIQUE(transaction_id, coin_id)
);

CREATE TABLE collections (
  id INTEGER,
  name TEXT,
  hash BLOB NOT NULL UNIQUE,
  description TEXT,
  metadata_id TEXT NOT NULL,
  is_visible BOOLEAN NOT NULL,
  minter_did BLOB NOT NULL,
  icon_url TEXT,
  banner_url TEXT,
  PRIMARY KEY (id)
);

CREATE TABLE nfts (
  id INTEGER,
  asset_id INTEGER NOT NULL UNIQUE,
  collection_id INTEGER,
  minter_did BLOB,
  owner_did BLOB,
  current_owner BLOB,
  is_owned BOOLEAN NOT NULL,
  is_sensitive_content BOOLEAN NOT NULL DEFAULT FALSE,
  metadata BLOB,
  metadata_updater_puzzle_hash BLOB,
  royalty_ten_thousandths INTEGER,
  royalty_puzzle_hash BLOB,
  metadata_hash BLOB,
  data_hash BLOB NOT NULL,
  license_hash BLOB NOT NULL,
  edition_number INTEGER,
  edition_total INTEGER,
  PRIMARY KEY (id),
  FOREIGN KEY (collection_id) REFERENCES collections(id) ON DELETE SET NULL,
  FOREIGN KEY (asset_id) REFERENCES assets(id) ON DELETE CASCADE
);

CREATE TABLE nft_data (
  id INTEGER,
  nft_id INTEGER NOT NULL,
  kind INTEGER NOT NULL,
  mime_type TEXT,
  is_hash_matched BOOLEAN NOT NULL,
  data_index TEXT NOT NULL,
  PRIMARY KEY (id),
  FOREIGN KEY (nft_id) REFERENCES nfts(id) ON DELETE CASCADE
);

CREATE TABLE tokens (
  id INTEGER,
  asset_id INTEGER NOT NULL UNIQUE,
  ticker TEXT,
  IsXch BOOLEAN GENERATED ALWAYS AS (asset_id = 0) STORED,
  PRIMARY KEY (id),
  FOREIGN KEY (asset_id) REFERENCES assets(id) ON DELETE CASCADE
);

CREATE TABLE dids (
  id INTEGER,
  asset_id INTEGER NOT NULL UNIQUE,
  is_owned BOOLEAN NOT NULL,
  metadata BLOB NOT NULL,
  recovery_list_hash BLOB,
  num_verifications_required BLOB NOT NULL,
  PRIMARY KEY (id),
  FOREIGN KEY (asset_id) REFERENCES assets(id) ON DELETE CASCADE
);

CREATE TABLE offer_coins (
  id INTEGER,
  offer_id INTEGER,
  coin_id INTEGER UNIQUE,
  PRIMARY KEY (id)
  FOREIGN KEY (offer_id) REFERENCES offers(id) ON DELETE CASCADE
  FOREIGN KEY (coin_id) REFERENCES coins(id) ON DELETE CASCADE
  UNIQUE(offer_id, coin_id)
);

CREATE TABLE future_did_names (
    launcher_id BLOB NOT NULL PRIMARY KEY,
    name TEXT NOT NULL
)