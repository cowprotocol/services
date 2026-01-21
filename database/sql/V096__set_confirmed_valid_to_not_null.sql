-- migrate any remaining orders
UPDATE orders
SET true_valid_to = COALESCE(
    (SELECT ethflow_orders.valid_to FROM ethflow_orders WHERE ethflow_orders.uid = orders.uid),
    orders.valid_to
);
-- at this point every order has the true_valid_to filled in
ALTER TABLE orders ALTER COLUMN true_valid_to SET NOT NULL;

--index on `true_valid_to` for quickly discarding expired orders
CREATE INDEX orders_true_valid_to ON orders USING btree (true_valid_to);
-- further drops the query from 100ms to 80ms (warmed cache)
CREATE INDEX okay_onchain_orders ON onchain_placed_orders USING btree (uid) WHERE placement_error IS NOT NULL;
