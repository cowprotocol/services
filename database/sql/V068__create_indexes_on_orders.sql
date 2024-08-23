CREATE INDEX order_creation_cancellation ON orders USING BTREE (creation_timestamp, cancellation_timestamp);
