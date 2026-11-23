-- Placement searches the unexpired quotes of an order's pair for one with
-- its side and fixed amount, the EVM quotes_token_expiration index.
CREATE INDEX CONCURRENTLY IF NOT EXISTS quotes_token_expiration
    ON solana.quotes USING BTREE (sell_token, buy_token, expiration_timestamp DESC);
