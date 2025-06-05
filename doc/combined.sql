-- Migration 0001: Initial Setup
CREATE TABLE `peaks` (
    `height` INTEGER NOT NULL PRIMARY KEY,
    `header_hash` BLOB NOT NULL
);

CREATE TABLE `derivations` (
    `p2_puzzle_hash` BLOB NOT NULL PRIMARY KEY,
    `index` INTEGER NOT NULL,
    `hardened` BOOLEAN NOT NULL,
    `synthetic_key` BLOB NOT NULL
);

CREATE INDEX `derivation_index` ON `derivations` (`index`, `hardened`);
CREATE INDEX `derivation_key` ON `derivations` (`synthetic_key`);

CREATE TABLE `coin_states` (
    `coin_id` BLOB NOT NULL PRIMARY KEY,
    `parent_coin_id` BLOB NOT NULL,
    `puzzle_hash` BLOB NOT NULL,
    `amount` BLOB NOT NULL,
    `spent_height` INTEGER,
    `created_height` INTEGER,
    `hint` BLOB,
    `synced` BOOLEAN NOT NULL,
    `transaction_id` BLOB,
    FOREIGN KEY (`transaction_id`) REFERENCES `transactions` (`transaction_id`) ON DELETE CASCADE
);

CREATE INDEX `coin_puzzle_hash` ON `coin_states` (`puzzle_hash`);
CREATE INDEX `coin_hint` ON `coin_states` (`hint`);
CREATE INDEX `coin_spent` ON `coin_states` (`spent_height`);
CREATE INDEX `coin_created` ON `coin_states` (`created_height`);
CREATE INDEX `coin_synced` ON `coin_states` (`synced`);
CREATE INDEX `coin_height` ON `coin_states` (`spent_height` ASC, `created_height` DESC);
CREATE INDEX `coin_transaction` ON `coin_states` (`transaction_id`);

CREATE TABLE `transactions` (
    `transaction_id` BLOB NOT NULL PRIMARY KEY,
    `aggregated_signature` BLOB NOT NULL,
    `fee` BLOB NOT NULL,
    `submitted_at` INTEGER
);

CREATE TABLE `transaction_spends` (
    `coin_id` BLOB NOT NULL PRIMARY KEY,
    `index` INTEGER NOT NULL,
    `transaction_id` BLOB NOT NULL,
    `parent_coin_id` BLOB NOT NULL,
    `puzzle_hash` BLOB NOT NULL,
    `amount` BLOB NOT NULL,
    `puzzle_reveal` BLOB NOT NULL,
    `solution` BLOB NOT NULL,
    FOREIGN KEY (`transaction_id`) REFERENCES `transactions` (`transaction_id`) ON DELETE CASCADE
);

CREATE INDEX `indexed_spend` ON `transaction_spends` (`transaction_id`, `index` ASC);

CREATE TABLE `p2_coins` (
    `coin_id` BLOB NOT NULL PRIMARY KEY,
    FOREIGN KEY (`coin_id`) REFERENCES `coin_states` (`coin_id`) ON DELETE CASCADE
);

CREATE TABLE `cats` (
    `asset_id` BLOB NOT NULL PRIMARY KEY,
    `name` TEXT,
    `ticker` TEXT,
    `visible` BOOLEAN NOT NULL,
    `icon` TEXT,
    `description` TEXT,
    `fetched` BOOLEAN NOT NULL,
    `is_named` BOOLEAN GENERATED ALWAYS AS (`name` IS NOT NULL) STORED
);

CREATE INDEX `cat_lookup` ON `cats` (`fetched`);
CREATE INDEX `cat_name` ON `cats` (`visible` DESC, `is_named` DESC, `name` ASC, `asset_id` ASC);

CREATE TABLE `cat_coins` (
    `coin_id` BLOB NOT NULL PRIMARY KEY,
    `parent_parent_coin_id` BLOB NOT NULL,
    `parent_inner_puzzle_hash` BLOB NOT NULL,
    `parent_amount` BLOB NOT NULL,
    `p2_puzzle_hash` BLOB NOT NULL,
    `asset_id` BLOB NOT NULL,
    FOREIGN KEY (`coin_id`) REFERENCES `coin_states` (`coin_id`) ON DELETE CASCADE
);

CREATE INDEX `cat_asset_id` ON `cat_coins` (`asset_id`);

CREATE TABLE `dids` (
    `launcher_id` BLOB NOT NULL PRIMARY KEY,
    `coin_id` BLOB NOT NULL,
    `name` TEXT,
    `visible` BOOLEAN NOT NULL,
    `is_owned` BOOLEAN NOT NULL,
    `is_named` BOOLEAN GENERATED ALWAYS AS (`name` IS NOT NULL) STORED,
    `created_height` INTEGER,
    `is_pending` BOOLEAN GENERATED ALWAYS AS (`created_height` IS NULL) STORED,
    FOREIGN KEY (`coin_id`) REFERENCES `did_coins` (`coin_id`) ON DELETE CASCADE
);

CREATE INDEX `did_coin_id` ON `dids` (`coin_id`);
CREATE INDEX `did_name` ON `dids` (`is_owned`, `visible` DESC, `is_pending` DESC, `is_named` DESC, `name` ASC, `launcher_id` ASC);

CREATE TABLE `future_did_names` (
    `launcher_id` BLOB NOT NULL PRIMARY KEY,
    `name` TEXT NOT NULL
);

CREATE TABLE `did_coins` (
    `coin_id` BLOB NOT NULL PRIMARY KEY,
    `parent_parent_coin_id` BLOB NOT NULL,
    `parent_inner_puzzle_hash` BLOB NOT NULL,
    `parent_amount` BLOB NOT NULL,
    `launcher_id` BLOB NOT NULL,
    `recovery_list_hash` BLOB,
    `num_verifications_required` BLOB NOT NULL,
    `metadata` BLOB NOT NULL,
    `p2_puzzle_hash` BLOB NOT NULL,
    FOREIGN KEY (`coin_id`) REFERENCES `coin_states` (`coin_id`) ON DELETE CASCADE
);

CREATE INDEX `did_launcher_id` ON `did_coins` (`launcher_id`);

CREATE TABLE `collections` (
    `collection_id` BLOB NOT NULL PRIMARY KEY,
    `did_id` BLOB NOT NULL,
    `metadata_collection_id` TEXT NOT NULL,
    `name` TEXT,
    `visible` BOOLEAN NOT NULL,
    `icon` TEXT,
    `is_named` BOOLEAN GENERATED ALWAYS AS (`name` IS NOT NULL) STORED
);

CREATE INDEX `col_name` ON `collections` (`visible` DESC, `is_named` DESC, `name` ASC, `collection_id` ASC);

CREATE TABLE `nfts` (
    `launcher_id` BLOB NOT NULL PRIMARY KEY,
    `coin_id` BLOB NOT NULL,
    `collection_id` BLOB,
    `minter_did` BLOB,
    `owner_did` BLOB,
    `visible` BOOLEAN NOT NULL,
    `sensitive_content` BOOLEAN NOT NULL,
    `name` TEXT,
    `is_owned` BOOLEAN NOT NULL,
    `is_named` BOOLEAN GENERATED ALWAYS AS (`name` IS NOT NULL) STORED,
    `created_height` INTEGER,
    `is_pending` BOOLEAN GENERATED ALWAYS AS (`created_height` IS NULL) STORED,
    `metadata_hash` BLOB,
    FOREIGN KEY (`coin_id`) REFERENCES `nft_coins` (`coin_id`) ON DELETE CASCADE
);

CREATE INDEX `nft_coin_id` ON `nfts` (`coin_id`);
CREATE INDEX `nft_metadata` ON `nfts` (`is_owned`, `metadata_hash`);
CREATE INDEX `nft_name` ON `nfts` (`is_owned`, `visible` DESC, `is_pending` DESC, `is_named` DESC, `name` ASC, `launcher_id` ASC);
CREATE INDEX `nft_recent` ON `nfts` (`is_owned`, `visible` DESC, `is_pending` DESC, `created_height` DESC, `launcher_id` ASC);
CREATE INDEX `nft_col_name` ON `nfts` (`is_owned`, `collection_id`, `visible` DESC, `is_pending` DESC, `is_named` DESC, `name` ASC, `launcher_id` ASC);
CREATE INDEX `nft_col_recent` ON `nfts` (`is_owned`, `collection_id`, `visible` DESC, `is_pending` DESC, `created_height` DESC, `launcher_id` ASC);

CREATE TABLE `nft_coins` (
    `coin_id` BLOB NOT NULL PRIMARY KEY,
    `parent_parent_coin_id` BLOB NOT NULL,
    `parent_inner_puzzle_hash` BLOB NOT NULL,
    `parent_amount` BLOB NOT NULL,
    `launcher_id` BLOB NOT NULL,
    `metadata` BLOB NOT NULL,
    `metadata_updater_puzzle_hash` BLOB NOT NULL,
    `current_owner` BLOB,
    `royalty_puzzle_hash` BLOB NOT NULL,
    `royalty_ten_thousandths` INTEGER NOT NULL,
    `p2_puzzle_hash` BLOB NOT NULL,
    `data_hash` BLOB,
    `metadata_hash` BLOB,
    `license_hash` BLOB,
    FOREIGN KEY (`coin_id`) REFERENCES `coin_states` (`coin_id`) ON DELETE CASCADE
);

CREATE INDEX `nft_launcher_id` ON `nft_coins` (`launcher_id`);

CREATE TABLE `nft_data` (
    `hash` BLOB NOT NULL PRIMARY KEY,
    `data` BLOB NOT NULL,
    `mime_type` TEXT NOT NULL
);

CREATE TABLE `nft_uris` (
    `uri` TEXT NOT NULL,
    `hash` BLOB NOT NULL,
    `checked` BOOLEAN NOT NULL,
    PRIMARY KEY (`uri`, `hash`)
);

CREATE INDEX `nft_uri_checked_hash` ON `nft_uris` (`checked`, `hash`);

-- Migration 0002: Offers
CREATE TABLE `offers` (
    `offer_id` BLOB NOT NULL PRIMARY KEY,
    `encoded_offer` TEXT NOT NULL,
    `expiration_height` INTEGER,
    `expiration_timestamp` BLOB,
    `fee` BLOB NOT NULL,
    `status` INTEGER NOT NULL,
    `inserted_timestamp` BLOB NOT NULL
);

CREATE INDEX `offer_status` ON `offers` (`status`);
CREATE INDEX `offer_timestamp` ON `offers` (`inserted_timestamp` DESC);

CREATE TABLE `offered_coins` (
    `offer_id` BLOB NOT NULL,
    `coin_id` BLOB NOT NULL,
    PRIMARY KEY (`offer_id`, `coin_id`),
    FOREIGN KEY (`offer_id`) REFERENCES `offers`(`offer_id`) ON DELETE CASCADE
);

CREATE INDEX `offer_coin_id` ON `offered_coins` (`coin_id`);

CREATE TABLE `offer_xch` (
    `offer_id` BLOB NOT NULL,
    `requested` BOOLEAN NOT NULL,
    `amount` BLOB NOT NULL,
    `royalty` BLOB NOT NULL,
    PRIMARY KEY (`offer_id`, `requested`),
    FOREIGN KEY (`offer_id`) REFERENCES `offers`(`offer_id`) ON DELETE CASCADE
);

CREATE INDEX `xch_offer_id` ON `offer_xch` (`offer_id`);

CREATE TABLE `offer_nfts` (
    `offer_id` BLOB NOT NULL,
    `requested` BOOLEAN NOT NULL,
    `launcher_id` BLOB NOT NULL,
    `royalty_puzzle_hash` BLOB NOT NULL,
    `royalty_ten_thousandths` INTEGER NOT NULL,
    `name` TEXT,
    `thumbnail` BLOB,
    `thumbnail_mime_type` TEXT,
    PRIMARY KEY (`offer_id`, `requested`),
    FOREIGN KEY (`offer_id`) REFERENCES `offers`(`offer_id`) ON DELETE CASCADE
);

CREATE INDEX `nft_offer_id` ON `offer_nfts` (`offer_id`);

CREATE TABLE `offer_cats` (
    `offer_id` BLOB NOT NULL,
    `requested` BOOLEAN NOT NULL,
    `asset_id` BLOB NOT NULL,
    `amount` BLOB NOT NULL,
    `royalty` BLOB NOT NULL,
    `name` TEXT,
    `ticker` TEXT,
    `icon` TEXT,
    PRIMARY KEY (`offer_id`, `requested`),
    FOREIGN KEY (`offer_id`) REFERENCES `offers`(`offer_id`) ON DELETE CASCADE
);

CREATE INDEX `cat_offer_id` ON `offer_cats` (`offer_id`);

-- Migration 0003: Offer Fix
CREATE TABLE `offer_nfts_2` (
    `offer_id` BLOB NOT NULL,
    `requested` BOOLEAN NOT NULL,
    `launcher_id` BLOB NOT NULL,
    `royalty_puzzle_hash` BLOB NOT NULL,
    `royalty_ten_thousandths` INTEGER NOT NULL,
    `name` TEXT,
    `thumbnail` BLOB,
    `thumbnail_mime_type` TEXT,
    PRIMARY KEY (`offer_id`, `launcher_id`, `requested`),
    FOREIGN KEY (`offer_id`) REFERENCES `offers`(`offer_id`) ON DELETE CASCADE
);

CREATE TABLE `offer_cats_2` (
    `offer_id` BLOB NOT NULL,
    `requested` BOOLEAN NOT NULL,
    `asset_id` BLOB NOT NULL,
    `amount` BLOB NOT NULL,
    `royalty` BLOB NOT NULL,
    `name` TEXT,
    `ticker` TEXT,
    `icon` TEXT,
    PRIMARY KEY (`offer_id`, `asset_id`, `requested`),
    FOREIGN KEY (`offer_id`) REFERENCES `offers`(`offer_id`) ON DELETE CASCADE
);

INSERT INTO `offer_nfts_2` SELECT * FROM `offer_nfts`;
INSERT INTO `offer_cats_2` SELECT * FROM `offer_cats`;

DROP TABLE `offer_nfts`;
DROP TABLE `offer_cats`;

ALTER TABLE `offer_nfts_2` RENAME TO `offer_nfts`;
ALTER TABLE `offer_cats_2` RENAME TO `offer_cats`;

CREATE INDEX `nft_offer_id` ON `offer_nfts` (`offer_id`);
CREATE INDEX `cat_offer_id` ON `offer_cats` (`offer_id`);

-- Migration 0004: Transactions
ALTER TABLE `coin_states` ADD COLUMN `kind` INTEGER NOT NULL DEFAULT 0;
CREATE INDEX `coin_kind` ON `coin_states` (`kind`);
CREATE INDEX `coin_kind_spent` ON `coin_states` (`kind`, `spent_height` ASC);

UPDATE `coin_states` SET `kind` = 1 WHERE `coin_id` IN (SELECT `coin_id` FROM `p2_coins`);
DROP TABLE `p2_coins`;

UPDATE `coin_states` SET `kind` = 2 WHERE `coin_id` IN (SELECT `coin_id` FROM `cat_coins`);
UPDATE `coin_states` SET `kind` = 3 WHERE `coin_id` IN (SELECT `coin_id` FROM `nft_coins`);
UPDATE `coin_states` SET `kind` = 4 WHERE `coin_id` IN (SELECT `coin_id` FROM `did_coins`);

-- Migration 0005: NFT Hash
ALTER TABLE `nft_uris` ADD COLUMN `hash_matches` BOOLEAN;

UPDATE `nft_uris` SET `hash_matches` = 1;

ALTER TABLE `nft_data` ADD COLUMN `hash_matches` BOOLEAN NOT NULL DEFAULT 1;

-- Migration 0006: NFT Search Indices
CREATE INDEX `nft_did_name` ON `nfts` (
    `is_owned`,
    `minter_did`,
    `visible` DESC,
    `is_pending` DESC,
    `is_named` DESC,
    `name` ASC,
    `launcher_id` ASC
);

CREATE INDEX `nft_did_recent` ON `nfts` (
    `is_owned`, 
    `minter_did`,
    `visible` DESC,
    `is_pending` DESC,
    `created_height` DESC,
    `launcher_id` ASC
);

CREATE VIRTUAL TABLE `nft_name_fts` USING fts5(
    name,
    nft_rowid UNINDEXED,
    launcher_id UNINDEXED
);

INSERT INTO nft_name_fts(name, nft_rowid, launcher_id)
SELECT name, rowid, launcher_id
FROM nfts
WHERE name IS NOT NULL AND name != '';

CREATE TRIGGER nfts_ai AFTER INSERT ON nfts BEGIN
  INSERT INTO nft_name_fts(name, nft_rowid, launcher_id)
  SELECT NEW.name, NEW.rowid, NEW.launcher_id
  WHERE NEW.name IS NOT NULL AND NEW.name != '';
END;

CREATE TRIGGER nfts_ad AFTER DELETE ON nfts BEGIN
  DELETE FROM nft_name_fts WHERE nft_rowid = OLD.rowid;
END;

CREATE TRIGGER nfts_au AFTER UPDATE ON nfts BEGIN
  DELETE FROM nft_name_fts WHERE nft_rowid = OLD.rowid;
  INSERT INTO nft_name_fts(name, nft_rowid, launcher_id)
  SELECT NEW.name, NEW.rowid, NEW.launcher_id
  WHERE NEW.name IS NOT NULL AND NEW.name != '';
END;

-- Migration 0007: Owner DID Indices
DROP INDEX `nft_did_name`;
DROP INDEX `nft_did_recent`;

CREATE INDEX `nft_minter_did_name` ON `nfts` (
    `is_owned`,
    `minter_did`,
    `visible` DESC,
    `is_pending` DESC,
    `is_named` DESC,
    `name` ASC,
    `launcher_id` ASC
);

CREATE INDEX `nft_minter_did_recent` ON `nfts` (
    `is_owned`, 
    `minter_did`,
    `visible` DESC,
    `is_pending` DESC,
    `created_height` DESC,
    `launcher_id` ASC
);

CREATE INDEX `nft_owner_did_name` ON `nfts` (
    `is_owned`,
    `owner_did`,
    `visible` DESC,
    `is_pending` DESC,
    `is_named` DESC,
    `name` ASC,
    `launcher_id` ASC
);

CREATE INDEX `nft_owner_did_recent` ON `nfts` (
    `is_owned`, 
    `owner_did`,
    `visible` DESC,
    `is_pending` DESC,
    `created_height` DESC,
    `launcher_id` ASC
);

-- Migration 0008: Duplicate Search Fix
DROP TRIGGER IF EXISTS nfts_ai;
DROP TRIGGER IF EXISTS nfts_ad;
DROP TRIGGER IF EXISTS nfts_au;

DELETE FROM nft_name_fts;

INSERT INTO nft_name_fts(name, nft_rowid, launcher_id)
SELECT name, rowid, launcher_id
FROM nfts
WHERE name IS NOT NULL 
AND name != '';

CREATE TRIGGER nfts_ai AFTER INSERT ON nfts BEGIN
  DELETE FROM nft_name_fts 
  WHERE launcher_id = NEW.launcher_id;
  
  INSERT INTO nft_name_fts(name, nft_rowid, launcher_id)
  SELECT NEW.name, NEW.rowid, NEW.launcher_id
  WHERE NEW.name IS NOT NULL 
  AND NEW.name != '';
END;

CREATE TRIGGER nfts_ad AFTER DELETE ON nfts BEGIN
  DELETE FROM nft_name_fts WHERE nft_rowid = OLD.rowid;
END;

CREATE TRIGGER nfts_au AFTER UPDATE ON nfts BEGIN
  DELETE FROM nft_name_fts WHERE launcher_id = NEW.launcher_id;
  
  INSERT INTO nft_name_fts(name, nft_rowid, launcher_id)
  SELECT NEW.name, NEW.rowid, NEW.launcher_id
  WHERE NEW.name IS NOT NULL 
  AND NEW.name != '';
END;

-- Migration 0009: Rust Migrations
CREATE TABLE `rust_migrations` (
    `version` INTEGER PRIMARY KEY
);

INSERT INTO `rust_migrations` (`version`) VALUES (0);

-- Migration 0010: Block Info
CREATE TABLE IF NOT EXISTS `blockinfo` (
    `height` INTEGER NOT NULL PRIMARY KEY,
    `unix_time` BIGINT NOT NULL
);

CREATE INDEX `blockinfo_index` ON `blockinfo` (`height`);

ALTER TABLE `coin_states` ADD COLUMN `spent_unixtime` BIGINT;
ALTER TABLE `coin_states` ADD COLUMN `created_unixtime` BIGINT;

-- Migration 0011: Thumbnails
CREATE TABLE `nft_thumbnails` (
    `hash` BLOB NOT NULL PRIMARY KEY,
    `icon` BLOB NOT NULL,
    `thumbnail` BLOB NOT NULL
);

DELETE FROM `nft_data`;
UPDATE `nft_uris` SET `checked` = 0;

-- Migration 0012: Drop FTS Tables
DROP TRIGGER IF EXISTS nfts_ai;
DROP TRIGGER IF EXISTS nfts_ad;
DROP TRIGGER IF EXISTS nfts_au;

DROP TABLE IF EXISTS nft_name_fts;

-- Migration 0013: NFT Edition
ALTER TABLE `nfts` ADD COLUMN `edition_number` INTEGER;
ALTER TABLE `nfts` ADD COLUMN `edition_total` INTEGER;

-- Migration 0014: NFT Edition Indexes
DROP INDEX IF EXISTS `nft_name`;
DROP INDEX IF EXISTS `nft_col_name`;
DROP INDEX IF EXISTS `nft_minter_did_name`;
DROP INDEX IF EXISTS `nft_owner_did_name`;

CREATE INDEX `nft_name` ON `nfts` (`is_owned`, `visible` DESC, `is_pending` DESC, `is_named` DESC, `name` ASC, `edition_number` ASC, `launcher_id` ASC);
CREATE INDEX `nft_col_name` ON `nfts` (`is_owned`, `collection_id`, `visible` DESC, `is_pending` DESC, `is_named` DESC, `name` ASC, `edition_number` ASC, `launcher_id` ASC);
CREATE INDEX `nft_minter_did_name` ON `nfts` (
    `is_owned`,
    `minter_did`,
    `visible` DESC,
    `is_pending` DESC,
    `is_named` DESC,
    `name` ASC,
    `edition_number` ASC,
    `launcher_id` ASC
);
CREATE INDEX `nft_owner_did_name` ON `nfts` (
    `is_owned`,
    `owner_did`,
    `visible` DESC,
    `is_pending` DESC,
    `is_named` DESC,
    `name` ASC,
    `edition_number` ASC,
    `launcher_id` ASC
); 