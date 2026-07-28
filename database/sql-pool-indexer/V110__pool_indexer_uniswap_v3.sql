-- Tracks the highest finalized block fully processed per factory contract.
-- A DB instance hosts a single network, so no `chain_id` column is needed.
CREATE TABLE pool_indexer_checkpoints (
    contract_address  BYTEA  NOT NULL,   -- factory address
    block_number      BIGINT NOT NULL,
    PRIMARY KEY (contract_address)
);

-- One row per pool, discovered from `PoolCreated` events. `factory`
-- partitions the table so multiple V3-compatible factories on the same
-- network can coexist (logs are fetched chain-wide, then partitioned at
-- the write boundary).
CREATE TABLE uniswap_v3_pools (
    address          BYTEA    NOT NULL,  -- pool address
    factory          BYTEA    NOT NULL,
    token0           BYTEA    NOT NULL,
    token1           BYTEA    NOT NULL,
    fee              INT      NOT NULL CHECK (fee > 0),  -- hundredths of a basis point (500 = 0.05%, 3000 = 0.3%, 10000 = 1%)
    token0_decimals  SMALLINT,
    token1_decimals  SMALLINT,
    token0_symbol    TEXT,
    token1_symbol    TEXT,
    created_block    BIGINT   NOT NULL,
    PRIMARY KEY (address)
);

-- Current state per pool. `sqrt_price_x96` + `tick` come from the latest
-- Swap/Initialize; `liquidity` + `block_number` also update on in-range
-- Mint/Burn events.
CREATE TABLE uniswap_v3_pool_states (
    pool_address    BYTEA   NOT NULL,
    block_number    BIGINT  NOT NULL,
    sqrt_price_x96  NUMERIC NOT NULL,   -- uint160
    liquidity       NUMERIC NOT NULL,   -- uint128
    -- `tick` here means the Uniswap V3 *price tick index* (signed
    -- int24), not a database index. See `uniswap_v3_ticks.tick_idx`.
    tick            INT     NOT NULL,
    PRIMARY KEY (pool_address),
    FOREIGN KEY (pool_address) REFERENCES uniswap_v3_pools(address)
);

-- Active ticks per pool. Rows with `liquidity_net = 0` are pruned.
-- `tick_idx` is the price tick coordinate (signed int24) — same domain
-- as `uniswap_v3_pool_states.tick`, one row per active tick boundary.
CREATE TABLE uniswap_v3_ticks (
    pool_address    BYTEA   NOT NULL,
    tick_idx        INT     NOT NULL,
    liquidity_net   NUMERIC NOT NULL,   -- int128 (can be negative)
    PRIMARY KEY (pool_address, tick_idx),
    FOREIGN KEY (pool_address) REFERENCES uniswap_v3_pools(address)
);

-- Symbol/decimals backfill hot paths. Partial on `IS NULL` so each
-- index shrinks to near-empty once most rows are populated (real value
-- or the `""` / `-1` "tried, failed" sentinel).
CREATE INDEX ON uniswap_v3_pools (token0) WHERE token0_symbol IS NULL;
CREATE INDEX ON uniswap_v3_pools (token1) WHERE token1_symbol IS NULL;
CREATE INDEX ON uniswap_v3_pools (token0) WHERE token0_decimals IS NULL;
CREATE INDEX ON uniswap_v3_pools (token1) WHERE token1_decimals IS NULL;
