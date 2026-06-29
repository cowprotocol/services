-- Tracks the highest finalized block fully processed per factory contract.
-- A DB instance hosts a single network, so no `chain_id` column is needed.
CREATE TABLE balancer_v2_checkpoints (
    factory_address  BYTEA  NOT NULL,   -- factory address
    block_number     BIGINT NOT NULL,
    PRIMARY KEY (factory_address)
);

-- One row per registered pool, discovered from each factory's `PoolCreated`
-- event. `pool_type` is derived from which factory created the pool (no
-- on-chain classification); the weighted V0/V3-plus distinction is recovered
-- from `factory`, so it isn't a separate type. `pool_type` is stored as the
-- string the API serves so there's no int<->enum mapping at the boundary.
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
-- near-empty once most rows are populated (real value or the `-1` "tried,
-- failed" sentinel).
CREATE INDEX ON balancer_v2_pool_tokens (token) WHERE decimals IS NULL;
