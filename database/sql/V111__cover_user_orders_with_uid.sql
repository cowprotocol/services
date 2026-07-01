-- The user_orders query computes the requested page of order uids with a
-- subquery `SELECT uid, creation_timestamp FROM orders WHERE owner = $1
-- ORDER BY creation_timestamp DESC`. The existing index
-- `user_order_creation_timestamp (owner, creation_timestamp DESC)` serves the
-- filter and ordering but does not contain `uid`, so each of the (up to)
-- OFFSET+LIMIT matched rows needs a heap fetch to read it.
--
-- Adding `uid` as an INCLUDE column makes that subquery index-only, removing
-- the per-row heap fetches for accounts with many orders. We replace the old
-- index with the covering one (CONCURRENTLY to avoid blocking writes).
CREATE INDEX CONCURRENTLY IF NOT EXISTS user_order_creation_timestamp_covering ON orders
    USING BTREE (owner, creation_timestamp DESC) INCLUDE (uid);

DROP INDEX CONCURRENTLY IF EXISTS user_order_creation_timestamp;
