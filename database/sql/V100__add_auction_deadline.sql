-- adds index on the deadline of an auction to quickly look up inflight orders from the db
CREATE INDEX CONCURRENTLY IF NOT EXISTS competition_auction_deadline ON competition_auctions USING BTREE(deadline);
