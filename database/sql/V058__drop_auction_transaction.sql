CREATE TYPE AuctionKind AS ENUM ('valid', 'invalid', 'unprocessed');

ALTER TABLE settlements
    DROP COLUMN tx_from,
    DROP COLUMN tx_nonce,
    ADD COLUMN auction_kind AuctionKind NOT NULL DEFAULT 'unprocessed',
    ADD COLUMN auction_id bigint;

DROP TABLE auction_transaction;