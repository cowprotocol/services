-- Add transaction hash a column to allow indexed retrieval.
-- Unique columns automatically gain a corresponding index.

ALTER TABLE solver_competitions
ADD COLUMN tx_hash bytea UNIQUE;

-- The `substr` call removes the `0x` prefix.
UPDATE solver_competitions
SET tx_hash = decode(substr(json ->> 'transactionHash', 3), 'hex');
