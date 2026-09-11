-- Order classes no longer exist: every order is a limit order (fee signed as zero, protocol fee
-- taken from the surplus) and JIT orders are identified by living in the `jit_orders` table.
-- The last producer of `market` rows was the ethflow indexer, which now records a
-- `non_zero_fee` placement error for such orders instead.
-- Dropping the column also drops the `orders_owner_class_valid_composite` index (replaced in V125).
ALTER TABLE orders DROP COLUMN class;
DROP TYPE OrderClass;
