# Schema Redesign Notes

## Relationships and Keys

### Primary and Foreign Keys

- Consistent naming for primary and foreign keys
  - Primary keys are always named `id`
  - Foreign keys are named `{table_name}_id`
  - Foreign keys are always nullable
  - Foreign keys are always indexed
  - Foreign keys are always a reference to the primary key of the referenced table

### Relationships

- Define all relationships in the schema using foreign keys and ensure they are indexed.

### Surrogate vs Natural Keys

The schema currently uses natural keys for relationships. Meaning that onchain values are used as primary and foreign keys. This is a good idea because it is easier to understand.

However, natural keys also come with some drawbacks:

- They couple the database to the onchain data so therefore encode business logic in the database
- They may impact performance since in our case they are BLOB types (larger indexes, less efficient comparison in joins)
- Limit the ability to reflect the data in a way that differs from the onchain data; ie representing multiple states of a coin at different points in time.
  
Surrogate keys are an alternative because they can be always unique and efficiently indexed. They also separate business logic from the database, allowing for more flexibility in relationship design. Integer based keys are a good choice because they are the most efficient and sqlite has in-built optimizations for them, reducing index size and improving performance in join comparisons and direct row lookups.

## Data Domains

### Coins

- Collapse `nft_coins`, `cat_coins`, `did_coins`, and `nft_coins` into a single `coins` table.
  - Add a `type` column to the table to differentiate between the different types of coins.
  - Add a `state` column to the table to represent the different states of a coin.
  - Add a `state_id` column to the table to represent the different states of a coin. (created? spent? are there other states)
  - Add as many columns as needed to represent the different attributes of the different types of coins (allowing them to be null when not applicable) - Basically the union of the all of the columns in the current set of `*_coins` tables
  - Add a `item_id` column to the table to join to the different attributes of the different types of coins

### NFTs

- Can the data, uri and thumbnail columns be collapsed into a single table?

### Offers

- Similar to coins, collapse `offer_nfts`, `offer_cats`, `offer_xch` into a single `offer_assets` table
- Any advantage to storing offers in a separate database and using linked tables?

### CATs

- Treat a CAT and XCH the same. From a modeling standpoint XCH seems to fit just fine into the `cats`.
- Maybe rename as `token`. XCH gets a special row but otherwise is the same. Should simplify some special case ahandling in the code for xch vs cat.
- This eliminates `offer_xch`
- (side note - maybe just download and keep the contents of what is currently the cats table in sync with dexie)
