-- Limit orders need to also store the solver_fee at the time of execution.
-- This is needed because solver_fee is not guaranteed to be constant over time for limit orders.

ALTER TABLE order_execution
ADD COLUMN solver_fee numeric(78, 0)
;
