-- Backfills trades.tx_hash (introduced in V112) for rows indexed before the
-- column existed.
--
-- A trade belongs to the settlement whose Settlement event is the first one
-- (lowest log index) after the trade's Trade event in the same block, so each
-- row is resolved by looking up exactly that settlements row. Trades without a
-- matching settlements row (e.g. gaps in event indexing) are left NULL; run
-- backfill_trades_tx_hash_check.sql beforehand to gauge how many such rows to
-- expect.
--
-- The update runs in batches over the primary key, committing after each one,
-- so it never holds long locks and can be aborted and re-run at any time
-- (already backfilled rows are skipped through the `tx_hash IS NULL` filter;
-- rows written by the event indexer since V112 already contain the hash).
--
-- Usage (per database, must run outside an explicit transaction so the DO
-- block can manage its own):
--   psql <connection options> -f backfill_trades_tx_hash.sql
--
-- Since every previously indexed row gets rewritten, consider running
-- `VACUUM ANALYZE trades;` once the backfill is done.

-- Abort (and re-run later) instead of queueing behind long-lived locks.
SET lock_timeout = '10s';

DO $$
DECLARE
    batch_size CONSTANT bigint := 50000;
    -- Exclusive lower bound of the current batch.
    cursor_block bigint := -1;
    cursor_log bigint := -1;
    -- Inclusive upper bound of the current batch (its last key).
    next_block bigint;
    next_log bigint;
    updated bigint;
    total bigint := 0;
BEGIN
    LOOP
        SELECT t.block_number, t.log_index INTO next_block, next_log
        FROM trades t
        WHERE (t.block_number, t.log_index) > (cursor_block, cursor_log)
        ORDER BY t.block_number, t.log_index
        OFFSET batch_size - 1
        LIMIT 1;

        IF next_block IS NULL THEN
            -- Fewer than batch_size rows left; process them without an upper
            -- bound.
            UPDATE trades t
            SET tx_hash = (
                SELECT s.tx_hash
                FROM settlements s
                WHERE s.block_number = t.block_number
                AND   s.log_index > t.log_index
                ORDER BY s.log_index
                LIMIT 1
            )
            WHERE (t.block_number, t.log_index) > (cursor_block, cursor_log)
            AND   t.tx_hash IS NULL;
        ELSE
            UPDATE trades t
            SET tx_hash = (
                SELECT s.tx_hash
                FROM settlements s
                WHERE s.block_number = t.block_number
                AND   s.log_index > t.log_index
                ORDER BY s.log_index
                LIMIT 1
            )
            WHERE (t.block_number, t.log_index) > (cursor_block, cursor_log)
            AND   (t.block_number, t.log_index) <= (next_block, next_log)
            AND   t.tx_hash IS NULL;
        END IF;

        GET DIAGNOSTICS updated = ROW_COUNT;
        total := total + updated;
        COMMIT;

        EXIT WHEN next_block IS NULL;
        RAISE NOTICE '% backfilled % rows so far (at block %)',
            clock_timestamp(), total, next_block;
        cursor_block := next_block;
        cursor_log := next_log;
    END LOOP;

    RAISE NOTICE 'done, backfilled % rows', total;
END
$$;
