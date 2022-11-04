-- The new fields can be null because they need to be back filled for old events.
ALTER TABLE settlements
    ADD tx_from bytea DEFAULT NULL,
    ADD tx_nonce bigint DEFAULT NULL,
    ADD CHECK ((tx_from IS NULL) = (tx_nonce IS NULL));

-- Technically one transaction could have multiple settlement events so from the perspective of the
-- settlements table this isn't a UNIQUE constraint, just like the existing column tx_hash.
CREATE INDEX settlements_tx_from_tx_nonce ON settlements (tx_from, tx_nonce);

-- Associate auction ids with the transaction that settled them.
--
-- The transaction address and nonce can be used to look up the hash through the
-- settlements table.
--
-- A transaction hash can be used to find the auction id by going in the other direction.
--
-- This table's constraints enforce a one to one relationship between auctions and transactions
-- which could technically be violated:
-- 1. One auction is settled through multiple `settle` calls.
-- 2. Two autions are settled in the same transaction (with any number of `settle` calls).
-- If we want to allow this then we need to change the constraints to mere indexes.
-- For now I decided to go with contraints because we expect this to hold from the perspective of
-- this table.
CREATE TABLE auction_transaction (
    auction_id bigint PRIMARY KEY,
    tx_from bytea NOT NULL,
    tx_nonce bigint NOT NULL,
    UNIQUE (tx_from, tx_nonce)
);

-- For new auctions we immediately create a row in auction_transaction.
-- Old auctions are  going to be inserted by the same task that updates old settlement events.
-- When all old auctions have been handled we are going to remove the tx_hash column from
-- solver_competitons.
