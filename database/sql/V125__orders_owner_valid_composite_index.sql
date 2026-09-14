-- Order classes no longer exist in the code: every order is a limit order (fee signed as zero,
-- protocol fee taken from the surplus) and JIT orders are identified by living in the
-- `jit_orders` table. The `class` column is no longer read or written by the services, but it
-- has to stay for one release so that the previous version of the orderbook (which still writes
-- it) and this version (which does not) can both insert orders while the deployment rolls over.
-- The default lets the new binaries insert without specifying the column. Dropping the column,
-- the `OrderClass` type and the old index happens in a follow-up migration once no writer of
-- the column is deployed anymore.
ALTER TABLE orders ALTER COLUMN class SET DEFAULT 'limit';

-- Replacement for `orders_owner_class_valid_composite`. The per-user
-- order counting query filters on owner, cancellation and true_valid_to only, so the class
-- column is no longer needed in the index. The old index is dropped together with the column in
-- the follow-up migration.
-- `CONCURRENTLY` cannot run inside a transaction and building the index on the large `orders`
-- tables takes longer than the deployment allows, so on production databases this index MUST
-- be created manually ahead of the deployment (same statement as below); `IF NOT EXISTS` then
-- turns this migration into a no-op there.
CREATE INDEX CONCURRENTLY IF NOT EXISTS orders_owner_valid_composite ON orders (owner, true_valid_to DESC) WHERE cancellation_timestamp IS NULL;

-- Rollback: `DROP INDEX CONCURRENTLY orders_owner_valid_composite; ALTER TABLE orders ALTER COLUMN class DROP DEFAULT;`
