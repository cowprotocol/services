CREATE INDEX order_cancellation_timestamp ON orders USING BTREE (cancellation_timestamp);
CREATE INDEX order_creation_cancellation ON orders USING BTREE (creation_timestamp, cancellation_timestamp);
