-- Which transaction cancelled the order PDA, so a cancellation that rolls
-- back before its slot finalizes is lifted again. NULL while the order is
-- live.
ALTER TABLE solana.order_pda
    ADD COLUMN cancelled_by_tx bytea CHECK (cancelled_by_tx IS NULL OR length(cancelled_by_tx) = 64);
