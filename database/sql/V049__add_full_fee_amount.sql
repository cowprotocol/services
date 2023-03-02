-- Limit orders need to also store the full_fee_amount at the time of execution.
-- This is needed because full_fee_amount is not guaranteed to be constant over time for limit orders.

ALTER TABLE order_execution
ADD COLUMN full_fee_amount numeric(78, 0)
;
