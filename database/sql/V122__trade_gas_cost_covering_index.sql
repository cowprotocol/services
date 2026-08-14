-- An order's gas cost is the sum of `gas_cost` over its fills. Keep that sum an
-- index-only scan by replacing the existing index with one containing the gas_cost.
CREATE INDEX CONCURRENTLY IF NOT EXISTS trades_covering_with_gas_cost ON trades (order_uid)
    INCLUDE (buy_amount, sell_amount, fee_amount, gas_cost);

DROP INDEX CONCURRENTLY IF EXISTS trades_covering;
