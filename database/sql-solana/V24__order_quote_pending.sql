-- Whether the order still waits for its quote decision. The indexer sets it on
-- an order created on chain. The orderbook clears it when it links the order's
-- quote or finds none, the autopilot clears it when the order waited too long.
-- A waiting order stays out of auctions.
ALTER TABLE solana.orders ADD COLUMN quote_pending boolean NOT NULL DEFAULT false;

-- The waiting orders, oldest first.
CREATE INDEX solana_orders_quote_pending ON solana.orders (creation_timestamp) WHERE quote_pending;
