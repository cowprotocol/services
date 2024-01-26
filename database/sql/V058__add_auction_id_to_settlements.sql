-- Step 1: Add the new column to the settlements table
ALTER TABLE settlements
    ADD COLUMN auction_id bigint;

-- Step 2: Populate auction_id columns

-- For all settlements that are not potentially still waiting to get indexed, set the auction ID to a default value of 0
-- Using 1000 blocks here to leave some margin for possible settlement updates that are still inflight.
UPDATE settlements
SET auction_id = 0
WHERE block_number < (SELECT max(block_number) FROM settlements) - 1000;

-- For all settlements that already have an auction_transaction record, set auction_id to the auction_transaction's auction_id
UPDATE settlements
SET auction_id = auction_transaction.auction_id
FROM auction_transaction
WHERE settlements.tx_from = auction_transaction.tx_from AND settlements.tx_nonce = auction_transaction.tx_nonce;

-- Step 3: (Once migration has been successful) Drop the auction_transaction table, and the tx_from and tx_nonce columns from the settlements table
