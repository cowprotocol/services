-- https://github.com/cowprotocol/services/issues/2100
-- Solver fee is now obsolete in favor or surplus_fee
ALTER TABLE order_execution 
  DROP COLUMN solver_fee;


-- https://github.com/cowprotocol/services/issues/2350
-- This table is no longer needed to match auctions with transactions
DROP TABLE auction_transaction;
