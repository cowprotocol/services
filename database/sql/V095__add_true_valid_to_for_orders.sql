ALTER TABLE orders ADD COLUMN true_valid_to bigint;

/*
The true_valid_to will have to be backfilled manually to ensure manageable load on db
using the following:
UPDATE orders
SET true_valid_to = COALESCE(
    (SELECT ethflow_orders.valid_to FROM ethflow_orders WHERE ethflow_orders.uid = orders.uid),
    orders.valid_to
)
WHERE uid IN (
    SELECT uid FROM orders
    WHERE true_valid_to IS NULL
    LIMIT 10000
);
*/

--index on `true_valid_to` for quickly discarding expired orders
CREATE INDEX orders_true_valid_to ON orders USING btree (true_valid_to);
-- further drops the query from 100ms to 80ms (warmed cache)
CREATE INDEX okay_onchain_orders ON onchain_placed_orders USING btree (uid) WHERE placement_error IS NOT NULL;
