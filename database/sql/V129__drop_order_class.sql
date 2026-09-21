-- Follow-up to V127 (replacement index) and V128 (column default).
--
-- Not reversible without data loss: the historic `market`/`liquidity` values are gone. Rollback
-- to the V128 state (all rows as `limit`), running the statements one at a time because
-- `CREATE INDEX CONCURRENTLY` cannot run inside a transaction block:
--   CREATE TYPE OrderClass AS ENUM ('market', 'liquidity', 'limit');
--   ALTER TABLE orders ADD COLUMN class OrderClass NOT NULL DEFAULT 'limit';
--   CREATE INDEX CONCURRENTLY orders_owner_class_valid_composite ON orders (owner, class, true_valid_to DESC) WHERE cancellation_timestamp IS NULL;
-- Postgres would drop this index implicitly with the column; dropping it explicitly documents
-- that `orders_owner_valid_composite` (V127) is the replacement.
DROP INDEX IF EXISTS orders_owner_class_valid_composite;
ALTER TABLE orders DROP COLUMN class;
DROP TYPE OrderClass;
