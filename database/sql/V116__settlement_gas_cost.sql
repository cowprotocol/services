-- Store the actual on-chain gas cost of each settlement transaction.
--
-- These values are read from the transaction receipt by the autopilot's
-- settlement observer), letting the orderbook attribute a real gas cost
-- to individual trades and orders.
--
-- Nullable: only populated for settlements observed after this migration is
-- deployed (no historical backfill).
ALTER TABLE settlements
    ADD COLUMN gas_used numeric(78, 0),
    ADD COLUMN effective_gas_price numeric(78, 0);
