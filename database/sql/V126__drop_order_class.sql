-- Follow-up to V125.
--
-- Not reversible without data loss: the historic `market`/`liquidity` values are gone. Rollback
-- to the V125 state (all rows as `limit`):
--   CREATE TYPE OrderClass AS ENUM ('market', 'liquidity', 'limit');
--   ALTER TABLE orders ADD COLUMN class OrderClass NOT NULL DEFAULT 'limit';
--   CREATE INDEX CONCURRENTLY orders_owner_class_valid_composite ON orders (owner, class, true_valid_to DESC) WHERE cancellation_timestamp IS NULL;
-- Postgres would drop this index implicitly with the column; dropping it explicitly documents
-- that `orders_owner_valid_composite` (V125) is the replacement.
DROP INDEX IF EXISTS orders_owner_class_valid_composite;
ALTER TABLE orders DROP COLUMN class;
DROP TYPE OrderClass;
