-- Balancer V2 discovery tables, applied on top of V110's uniswap_v3_* schema in
-- the pool-indexer's per-network DB. Checkpoints reuse `pool_indexer_checkpoints`
-- (keyed by factory address): Balancer and Uniswap V3 factory addresses are
-- distinct contracts, so both protocols share that table without collision.

-- One row per registered pool, discovered from each factory's `PoolCreated`
-- event. `pool_type` is derived from which factory created the pool (no on-chain
-- classification); weighted V0 and V3-plus both map to `Weighted` (the variant
-- is recoverable from `factory`). Stored as the string the API serves, so
-- there's no int<->enum mapping at the boundary.
CREATE TABLE balancer_v2_pools (
    pool_id        BYTEA  NOT NULL,   -- 32-byte Balancer poolId
    address        BYTEA  NOT NULL,   -- pool address (poolId's first 20 bytes)
    factory        BYTEA  NOT NULL,
    pool_type      TEXT   NOT NULL CHECK (pool_type IN ('Weighted', 'Stable', 'ComposableStable', 'LiquidityBootstrapping')),
    created_block  BIGINT NOT NULL,
    PRIMARY KEY (pool_id)
);

-- Tokens per pool, in `Vault.getPoolTokens` order (`position`). `decimals` is
-- nullable and filled in by the backfill task. `weight` is the Balancer Bfp
-- (1e18 fixed-point) normalized weight, set only for weighted pools; NULL for
-- stable/composable-stable/LBP (their weights are absent or fetched on-chain).
CREATE TABLE balancer_v2_pool_tokens (
    pool_id   BYTEA    NOT NULL,
    position  INT      NOT NULL,
    token     BYTEA    NOT NULL,
    decimals  SMALLINT,             -- NULL = not yet fetched; -1 = fetched but call failed
    weight    NUMERIC,              -- Bfp (1e18); weighted pools only, else NULL
    PRIMARY KEY (pool_id, position),
    FOREIGN KEY (pool_id) REFERENCES balancer_v2_pools(pool_id)
);

-- Decimals backfill hot path. Partial on `IS NULL` so the index shrinks to
-- near-empty once most rows are populated (real value or the `-1` sentinel).
CREATE INDEX ON balancer_v2_pool_tokens (token) WHERE decimals IS NULL;
