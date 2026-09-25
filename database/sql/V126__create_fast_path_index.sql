-- Partial index over every fast-path order. Direct users today:
-- * the autopilot startup scan that re-notifies fast-path orders whose
--   `valid_from` hasn't been populated yet (`WHERE fast_path AND
--   valid_from IS NULL` — the extra predicate filters the small
--   pending slice from within the index scan);
-- * ad-hoc queries about fast-path orders (per-user history, analytics,
--   audits) that would otherwise have to seq-scan `orders`.
--
-- Deliberately broader than the startup scan alone so we don't need a
-- second index once fast-path analytics show up. If fast-path ever
-- becomes the norm (>50% of orders) this should be re-evaluated
-- against a plain btree.
CREATE INDEX CONCURRENTLY IF NOT EXISTS orders_fast_path
    ON orders (uid) WHERE fast_path;
