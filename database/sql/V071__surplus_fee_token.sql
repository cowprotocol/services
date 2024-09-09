-- Add a new column to the order_execution table to store the surplus fee token
ALTER TABLE order_execution 
ADD COLUMN surplus_fee_token bytea NOT NULL DEFAULT '\x0000000000000000000000000000000000000000';

-- Now populate existing rows with the sell token taken from the 'trades' table, JOIN-ing on the order uid.
UPDATE order_execution oe
SET surplus_fee_token = COALESCE(t.sell_token, '\x0000000000000000000000000000000000000000')
FROM trades t
WHERE oe.order_uid = t.order_uid
OR NOT EXISTS (SELECT 1 FROM trades WHERE order_uid = oe.order_uid);

-- Drop the default value for the surplus_fee_token column
ALTER TABLE order_execution ALTER COLUMN surplus_fee_token DROP DEFAULT;
