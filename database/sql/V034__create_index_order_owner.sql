-- replace index for effective order searching
DROP INDEX user_order_creation_timestamp;

CREATE INDEX order_owner ON orders USING HASH (owner);
