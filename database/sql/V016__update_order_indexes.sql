-- The following index already allows indexing by owner.
DROP INDEX order_owner;

-- The index can be traversed in both directions anyway but we usually fetch user orders with this
-- ordering so using it here too.
CREATE INDEX user_order_creation_timestamp ON orders USING BTREE (owner, creation_timestamp DESC);
