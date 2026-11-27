-- Partial index over the wait-for-CoW orders whose `valid_from` the autopilot
-- hasn't set yet. Every run-loop iteration updates exactly these rows, which
-- only stay in the index until that update, so it stays small.
--
-- Rollback: `DROP INDEX CONCURRENTLY IF EXISTS orders_wait_for_cow_pending;`
CREATE INDEX CONCURRENTLY IF NOT EXISTS orders_wait_for_cow_pending
    ON orders (uid) WHERE wait_for_cow AND valid_from IS NULL;
