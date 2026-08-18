-- Each trade's share of its settlement's gas cost: `gas_used *
-- effective_gas_price` (V116) split equally between the trades that settlement
-- settled for a user. Trades of JIT orders that only provide liquidity get 0.
-- Stored so order and trade lookups don't re-derive it per fill.
--
-- Nullable: not backfilled, so only settlements observed after this migration.
ALTER TABLE trades
    ADD COLUMN gas_cost numeric(78, 0);
