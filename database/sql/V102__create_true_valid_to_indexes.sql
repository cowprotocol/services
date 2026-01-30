--index on `true_valid_to` for quickly discarding expired orders
CREATE INDEX CONCURRENTLY IF NOT EXISTS orders_true_valid_to ON orders USING btree (true_valid_to);
-- further drops the query from 100ms to 80ms (warmed cache)
CREATE INDEX CONCURRENTLY IF NOT EXISTS okay_onchain_orders ON onchain_placed_orders USING btree (uid) WHERE placement_error IS NOT NULL;
