-- Tracks the highest finalized block fully processed per chain+contract
CREATE TABLE pool_indexer_checkpoints (
    chain_id       BIGINT NOT NULL,
    contract       BYTEA  NOT NULL,      -- factory or pool address
    block_number   BIGINT NOT NULL,
    PRIMARY KEY (chain_id, contract)
);

-- One row per discovered pool (from PoolCreated events on the factory).
-- `factory` is the emitting factory's address; it partitions the table so each
-- indexer writes only to its own rows on chains where multiple V3-compatible
-- factories are configured (same chain's logs are fetched chain-wide).
CREATE TABLE uniswap_v3_pools (
    chain_id         BIGINT   NOT NULL,
    address          BYTEA    NOT NULL,  -- pool address
    factory          BYTEA    NOT NULL,
    token0           BYTEA    NOT NULL,
    token1           BYTEA    NOT NULL,
    fee              INT      NOT NULL,  -- hundredths of a basis point (500 = 0.05%, 3000 = 0.3%, 10000 = 1%)
    token0_decimals  SMALLINT,
    token1_decimals  SMALLINT,
    token0_symbol    TEXT,
    token1_symbol    TEXT,
    created_block    BIGINT   NOT NULL,
    PRIMARY KEY (chain_id, address)
);

-- Current state of each pool (updated on every Swap or Initialize)
CREATE TABLE uniswap_v3_pool_states (
    chain_id        BIGINT  NOT NULL,
    pool_address    BYTEA   NOT NULL,
    block_number    BIGINT  NOT NULL,
    sqrt_price_x96  NUMERIC NOT NULL,   -- uint160
    liquidity       NUMERIC NOT NULL,   -- uint128
    tick            INT     NOT NULL,
    PRIMARY KEY (chain_id, pool_address),
    FOREIGN KEY (chain_id, pool_address) REFERENCES uniswap_v3_pools(chain_id, address)
);

-- Active ticks per pool (rows with liquidity_net = 0 are pruned)
CREATE TABLE uniswap_v3_ticks (
    chain_id        BIGINT  NOT NULL,
    pool_address    BYTEA   NOT NULL,
    tick_idx        INT     NOT NULL,
    liquidity_net   NUMERIC NOT NULL,   -- int128 (can be negative)
    PRIMARY KEY (chain_id, pool_address, tick_idx),
    FOREIGN KEY (chain_id, pool_address) REFERENCES uniswap_v3_pools(chain_id, address)
);

-- Symbol backfill hot paths: both `get_tokens_missing_symbols` (scan for NULL
-- symbols) and `set_token_symbol` (update by token address where symbol IS
-- NULL) hit these. Partial on the IS NULL predicate so the indices shrink to
-- near-empty once most symbols are populated.
CREATE INDEX ON uniswap_v3_pools (chain_id, token0) WHERE token0_symbol IS NULL;
CREATE INDEX ON uniswap_v3_pools (chain_id, token1) WHERE token1_symbol IS NULL;
