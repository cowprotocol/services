-- When the owner cancelled the order through the orderbook, with nothing on
-- chain. Read only while no order PDA exists: once a creation lands, the PDA's
-- cancellation_timestamp is the cancellation.
ALTER TABLE solana.orders ADD COLUMN cancelled_at timestamp with time zone;
