-- index to efficiently find auctions which contained a given order
CREATE INDEX CONCURRENTLY IF NOT EXISTS competition_auctions_order_uids_gin
    ON competition_auctions USING GIN (order_uids);
