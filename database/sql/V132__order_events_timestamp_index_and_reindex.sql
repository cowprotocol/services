-- Supports the periodic `DELETE FROM order_events WHERE timestamp < $1` cleanup,
-- which otherwise has to scan the whole table.
CREATE INDEX CONCURRENTLY IF NOT EXISTS order_events_by_timestamp ON order_events USING BTREE (timestamp);

-- `order_events_by_uid` is mostly empty pages left over from the high event
-- volume before the cleanup and event deduplication were introduced. B-tree
-- indexes never return empty pages to the OS, so it has to be rebuilt to
-- reclaim the space.
REINDEX INDEX CONCURRENTLY order_events_by_uid;
