-- Ties order rows to the transaction that created them, so a creation that
-- rolls back before its slot finalizes can be reverted. NULL on rows written
-- before this migration, which the audit treats as already final.
-- Creation provenance next to created_by, so a creation that rolls back
-- before its slot finalizes can be found and marked. NULL on rows written
-- before this migration, which the audit treats as already final.
-- `is_reorged` set means the creation rolled back: the rows stay as the
-- audit trail, every reader skips the order, and a re-landed creation
-- clears the flag.
ALTER TABLE solana.order_pda
    ADD COLUMN created_by_tx bytea CHECK (created_by_tx IS NULL OR length(created_by_tx) = 64),
    ADD COLUMN created_in_slot bigint,
    ADD COLUMN is_reorged boolean NOT NULL DEFAULT false;

-- The finalization audit scans each table by slot range.
CREATE INDEX solana_order_pda_created_in_slot ON solana.order_pda (created_in_slot)
    WHERE created_in_slot IS NOT NULL;
CREATE INDEX solana_settlements_slot ON solana.settlements (slot);
CREATE INDEX solana_dead_letter_slot ON solana.dead_letter (slot);
