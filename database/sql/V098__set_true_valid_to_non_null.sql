-- migrate any remaining orders
UPDATE orders
SET true_valid_to = COALESCE(
    (SELECT ethflow_orders.valid_to FROM ethflow_orders WHERE ethflow_orders.uid = orders.uid),
    orders.valid_to
);
-- at this point every order has the true_valid_to filled in
ALTER TABLE orders ALTER COLUMN true_valid_to SET NOT NULL;
