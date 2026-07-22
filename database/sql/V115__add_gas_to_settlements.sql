-- Store the actual on-chain gas cost of each settlement transaction.
--
-- These values are read from the transaction receipt by the autopilot's
-- settlement observer (gas_used and effective_gas_price are computed there but,
-- since the `settlement_observations` table was dropped in V090, no longer
-- persisted). Re-introducing them here lets the orderbook attribute a real gas
-- cost to individual trades and orders.
--
-- Nullable: only populated for settlements observed after this migration is
-- deployed (no historical backfill).
ALTER TABLE settlements
    ADD COLUMN gas_used numeric(78, 0),
    ADD COLUMN effective_gas_price numeric(78, 0);
