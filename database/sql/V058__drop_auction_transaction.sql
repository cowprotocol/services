-- Step 1: Add the new columns to the settlements table
CREATE TYPE AuctionKind AS ENUM ('valid', 'invalid', 'unprocessed');

ALTER TABLE settlements
    ADD COLUMN auction_kind AuctionKind NOT NULL DEFAULT 'unprocessed',
    ADD COLUMN auction_id bigint;

-- Step 2a: Populate auction_kind and auction_id columns
-- For all settlements that already have an auction_transaction record, set auction_kind to 'valid' and auction_id to the auction_transaction's auction_id
UPDATE settlements
SET auction_kind = 'valid',
    auction_id = auction_transaction.auction_id
FROM auction_transaction
WHERE settlements.tx_from = auction_transaction.tx_from AND settlements.tx_nonce = auction_transaction.tx_nonce;

-- Step 2b: Populate auction_kind and auction_id columns
-- For all settlements that have auction_id = NULL, set auction_kind to 'invalid'
UPDATE settlements
SET auction_kind = 'invalid'
WHERE auction_id IS NULL;

-- Step 2c: Populate auction_kind and auction_id columns
-- For all settlements, going from the most recent to the oldest, set auction_kind to 'unprocessed' until the first settlement with auction_kind = 'valid' is found
UPDATE settlements
SET
    auction_kind = 'unprocessed'
WHERE
    auction_kind = 'invalid'
    AND block_number > (
        SELECT MAX(block_number) 
            FROM settlements
            WHERE auction_kind = 'valid'
    );

-- Step 4: Drop the auction_transaction table, and the tx_from and tx_nonce columns from the settlements table
DROP TABLE auction_transaction;
ALTER TABLE settlements
    DROP COLUMN tx_from,
    DROP COLUMN tx_nonce;
