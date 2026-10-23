-- The periodic cleanup deletes `order_events` rows by `timestamp` alone, which
-- `order_events_by_uid` can't serve since `timestamp` isn't its leading
-- column, so every cleanup scans the whole table.
CREATE INDEX CONCURRENTLY IF NOT EXISTS order_events_by_timestamp ON order_events USING BTREE (timestamp);
