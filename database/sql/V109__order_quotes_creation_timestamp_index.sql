-- The job replicating data to the analytics DB fetches quotes based on the creation timestamp.
-- This query significantly speeds that up.
CREATE INDEX CONCURRENTLY IF NOT EXISTS order_quotes_creation_timestamp ON order_quotes USING BTREE(creation_timestamp);
