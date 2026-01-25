-- adds index on the deadline of an auction to quickly look up inflight orders from the db
CREATE INDEX CONCURENTLY competition_auction_deadline ON auction_competitions USING BTREE(deadline);
