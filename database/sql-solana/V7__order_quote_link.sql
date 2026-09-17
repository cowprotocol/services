-- The quote a sponsored order was placed against, when the creation body
-- named one and it matched the order. Orders created on chain never carry
-- one.
ALTER TABLE solana.orders
    ADD COLUMN quote_id bigint;
