-- Speeds up the `valid_from` filter in the solvable-orders lookups. Only orders
-- that actually set a `valid_from` are indexed; the vast majority are NULL.
CREATE INDEX CONCURRENTLY IF NOT EXISTS orders_valid_from ON orders USING btree (valid_from) WHERE valid_from IS NOT NULL;
