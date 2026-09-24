-- Adds a covering variant of the trades owner substring index that includes
-- `order_uid` to speed up the account trade history query.
--
-- The old `trades_order_uid_owner` index becomes redundant once this exists —
-- the planner always prefers the covering one — so we drop it in the same
-- migration.

CREATE INDEX IF NOT EXISTS trades_owner_covering ON trades (
    substring(order_uid, 33, 20),
    block_number DESC,
    log_index DESC
) INCLUDE (order_uid);

DROP INDEX IF EXISTS trades_order_uid_owner;
