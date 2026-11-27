-- The quote the order's creation transaction names in an SPL Memo instruction,
-- as the text `cow:quote:<id>` with the id in decimal. NULL when the
-- transaction carries no such memo, more than one, or more than one creation.
ALTER TABLE solana.order_pda ADD COLUMN quote_id bigint;
