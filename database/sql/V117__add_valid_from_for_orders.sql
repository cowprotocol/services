-- `valid_from` is the earliest UNIX timestamp (in seconds) at which an order may
-- enter a batch auction. NULL means no lower bound, i.e. the order is eligible
-- immediately (the behaviour for all existing orders), so no backfill is needed.
ALTER TABLE orders ADD COLUMN valid_from bigint;
