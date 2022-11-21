-- Like orders.surplus_fee but is the value used to execute a limit order in a specific auction.
-- Frontend requested to have access to this.

ALTER TABLE order_rewards
RENAME TO order_execution
;

ALTER TABLE order_execution
ALTER COLUMN reward DROP NOT NULL,
ADD COLUMN surplus_fee numeric(78, 0)
;
