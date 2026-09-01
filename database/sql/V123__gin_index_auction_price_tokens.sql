-- Index to efficiently find the most recent auction that priced a given token.
-- Without it, looking up a token that is absent from the newest auctions
-- degenerates into a backward scan of `competition_auctions` that detoasts
-- `price_tokens` on every row.
CREATE INDEX CONCURRENTLY IF NOT EXISTS competition_auctions_price_tokens_gin
    ON competition_auctions USING GIN (price_tokens);
