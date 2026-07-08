-- Pre-flight check for backfill_trades_tx_hash.sql.
--
-- Reports how consistent the trades <-> settlements association is before
-- running the backfill:
--   * trades_without_settlement: trades that have no settlement event after
--     them in the same block, i.e. the rows the backfill would leave NULL
--     (e.g. gaps in event indexing).
--   * settlements_without_trades: settlements that no trade resolves to.
--     Not necessarily an indexing gap: settle() calls with an empty trades
--     array legitimately emit no Trade events.
-- The block ranges show where the unmatched rows cluster.
--
-- Does not touch trades.tx_hash, so it also runs before the V112 migration
-- is applied.
--
-- Read-only; safe to run against the read replica. Scans both tables once
-- with an index probe per row, so expect it to take a few minutes on the
-- bigger databases.

WITH unmatched_trades AS (
    SELECT t.block_number
    FROM trades t
    WHERE NOT EXISTS (
        SELECT 1
        FROM settlements s
        WHERE s.block_number = t.block_number
    )
),
unmatched_settlements AS (
    SELECT s.block_number
    FROM settlements s
    WHERE NOT EXISTS (
        SELECT 1
        FROM trades t
        WHERE t.block_number = s.block_number
        AND   t.log_index < s.log_index
        -- only trades after the previous settlement in the same block
        -- resolve to s
        AND   t.log_index > COALESCE((
            SELECT max(prev.log_index)
            FROM settlements prev
            WHERE prev.block_number = s.block_number
            AND   prev.log_index < s.log_index
        ), -1)
    )
)
SELECT
    (SELECT count(*) FROM unmatched_trades) AS trades_without_settlement,
    (SELECT min(block_number) FROM unmatched_trades) AS first_unmatched_trade_block,
    (SELECT max(block_number) FROM unmatched_trades) AS last_unmatched_trade_block,
    (SELECT count(*) FROM unmatched_settlements) AS settlements_without_trades,
    (SELECT min(block_number) FROM unmatched_settlements) AS first_unmatched_settlement_block,
    (SELECT max(block_number) FROM unmatched_settlements) AS last_unmatched_settlement_block;
