-- Replacement for `orders_owner_class_valid_composite`.
--
-- `CONCURRENTLY` cannot run inside a transaction, which is also why this statement has to live in
-- its own migration file (Flyway refuses to mix transactional and non-transactional statements).
-- Building the index on the large `orders` tables takes longer than the deployment allows, so on
-- production databases this index MUST be created manually ahead of the deployment (same
-- statement as below); `IF NOT EXISTS` then turns this migration into a no-op there.
--
-- Rollback: `DROP INDEX CONCURRENTLY orders_owner_valid_composite;`
CREATE INDEX CONCURRENTLY IF NOT EXISTS orders_owner_valid_composite ON orders (owner, true_valid_to DESC) WHERE cancellation_timestamp IS NULL;
