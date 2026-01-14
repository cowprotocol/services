ALTER TABLE orders ADD COLUMN confirmed_valid_to bigint;
UPDATE orders SET confirmed_valid_to = COALESCE(
    (SELECT ethflow_orders.valid_to FROM ethflow_orders WHERE ethflow_orders.uid = orders.uid),
    orders.valid_to
);
ALTER TABLE orders ALTER COLUMN confirmed_valid_to SET NOT NULL;

CREATE INDEX orders_owner_live_limit 
ON orders USING btree (owner, confirmed_valid_to)
WHERE cancellation_timestamp IS NULL 
  AND class = 'limit';