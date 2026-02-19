-- create a separate index for cancellation_timestamp to improve queries
-- filtering based on the cancellation_timestamp
-- no extra index with only the creation_timestamp needs to be created
-- since that already exists.
CREATE INDEX CONCURRENTLY order_cancellation_timestamp ON orders USING BTREE(cancellation_timestamp);

