-- To optimize the performance of the user_orders query, we introduce a new index that allows
-- us to quickly get the latest orders from a owner
CREATE INDEX user_order_creation_timestamp ON orders USING BTREE (owner, creation_timestamp DESC);

