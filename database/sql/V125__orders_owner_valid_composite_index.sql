-- Replacement for `orders_owner_class_valid_composite`, which is dropped together with the
-- `class` column in V126. The per-user order counting query filters on owner, cancellation
-- and true_valid_to only, so the class column is no longer needed in the index.
-- `CONCURRENTLY` cannot run inside a transaction; `IF NOT EXISTS` allows creating the index
-- manually ahead of the deployment on large databases.
CREATE INDEX CONCURRENTLY IF NOT EXISTS orders_owner_valid_composite ON orders (owner, true_valid_to DESC) WHERE cancellation_timestamp IS NULL;
