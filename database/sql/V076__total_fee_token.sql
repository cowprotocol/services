ALTER TABLE order_execution
RENAME COLUMN surplus_fee TO executed_fee;

-- Add a new column to the order_execution table to store the executed fee token
ALTER TABLE order_execution
ADD COLUMN executed_fee_token bytea NOT NULL DEFAULT '\x0000000000000000000000000000000000000000';

-- Now populate existing rows with the sell token taken from the 'orders' table, or if it doesn't exist, try from 'jit_orders'.
UPDATE order_execution oe
SET executed_fee_token = COALESCE(
    sub.sell_token, '\x0000000000000000000000000000000000000000'
)
FROM (
    SELECT o.uid, o.sell_token
    FROM orders o
    UNION
    SELECT j.uid, j.sell_token
    FROM jit_orders j
    WHERE NOT EXISTS (SELECT 1 FROM orders WHERE orders.uid = j.uid)
) AS sub
WHERE oe.order_uid = sub.uid;

-- Drop the default value for the executed_fee_token column
ALTER TABLE order_execution ALTER COLUMN executed_fee_token DROP DEFAULT;
