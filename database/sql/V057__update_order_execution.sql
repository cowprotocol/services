-- solver_fee was added in V049__add_full_fee_amount.sql
-- Limit order's fee is no longer calculated beforehand, but provided by the solvers at the time of solving. However, under colocation, this fee is not reported to the autopilot but calculated from the onchain calldata.

ALTER TABLE order_execution 
  DROP COLUMN solver_fee;

-- Rename surplus_fee to executed_fee as it will represent the general fee paid to the solvers due to network fees.
-- For market orders, this fee can be fetched from the orders table (but we can keep the duplicate here for completness).
-- For limit orders, fee is expected to be calculated after the transaction mined, and then this field will be updated from autopilot.

ALTER TABLE order_execution
  RENAME surplus_fee TO executed_fee;
