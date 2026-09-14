-- Backs the solvable-orders exclusion `NOT (fast_path AND valid_from IS NULL)`
-- and the autopilot startup scan that re-notifies fast-path orders whose
-- `valid_from` hasn't been populated yet. Only the (tiny) subset of pending
-- fast-path rows is indexed.
CREATE INDEX CONCURRENTLY IF NOT EXISTS orders_pending_fast_path
    ON orders (uid) WHERE fast_path AND valid_from IS NULL;
