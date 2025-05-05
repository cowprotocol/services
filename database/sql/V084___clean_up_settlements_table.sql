-- 1. Drop the check constraint that references tx_from and tx_nonce
ALTER TABLE settlements
    DROP CONSTRAINT settlements_check;

-- 2. Drop the tx_from column
ALTER TABLE settlements
    DROP COLUMN tx_from;

-- 3. Drop the tx_nonce column
ALTER TABLE settlements
    DROP COLUMN tx_nonce;
