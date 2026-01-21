-- This migration should only be applied when the manual steps in
-- V095__add_true_valid_to_for_orders.sql have been completed.
ALTER TABLE orders ALTER COLUMN true_valid_to SET NOT NULL;

--index on `true_valid_to` for quickly discarding expired orders
CREATE INDEX orders_true_valid_to ON orders USING btree (true_valid_to);
-- further drops the query from 100ms to 80ms (warmed cache)
CREATE INDEX okay_onchain_orders ON onchain_placed_orders USING btree (uid) WHERE placement_error IS NOT NULL;
