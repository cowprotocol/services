-- These field are not needed anymore since they were used to calculate the 
-- surplus fee for limit orders beforehand. Now, since the surplus fee 
-- is provided by the solvers at the time of solving, we can remove these feilds 
-- and rely on the order_execution table to get the executed surplus fee.

ALTER TABLE orders 
  DROP COLUMN surplus_fee,
  DROP COLUMN surplus_fee_timestamp;
