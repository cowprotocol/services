--- Add true_valid_to column which will uniformly store all orders' validity
--- This will enable creating an index to speedup user_orders_with_quote and solvable_orders queries
--- which have increased in runtime since a big influx of ethflow orders
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