# Plan: Replace Subgraphs with Purpose-Built Pool Indexer

## Context

The driver currently bootstraps Uniswap V3 and Balancer V2 liquidity via The Graph subgraphs. Subgraphs are queried once at startup to get pool state at a safe historical block; on-chain events then maintain the state. The problem: subgraph dependency is unreliable (rate limits, outages, versioning, chain support gaps). This plan replaces the subgraph with an in-house indexer service that reads directly from the chain, persists state in Postgres, and exposes a REST API.

Scope: **Uniswap V3 only** to start. Balancer V2 follows the same pattern and can be added later.

Finalized blocks only — no reorg handling required.

---

## New Crate: `crates/pool-indexer`

Follows the same lib + binary pattern as `orderbook`, `autopilot`, etc.

```
crates/pool-indexer/
  Cargo.toml
  src/
    main.rs            → pool_indexer::start(std::env::args())
    lib.rs             → pub mod declarations, pub use run::{run, start}
    run.rs             → parse args, load config, start DB, start indexer, start HTTP server
    arguments.rs       → clap: --config <path>
    config.rs          → TOML config struct (serde)
    indexer/
      mod.rs
      uniswap_v3.rs    → event loop: poll finalized block, fetch logs, write to DB
    db/
      mod.rs
      uniswap_v3.rs    → sqlx queries (pools, ticks, state, indexed_block)
    api/
      mod.rs           → axum Router
      uniswap_v3.rs    → REST handlers
```

Also needs DB migration files under `database/sql/` following the Flyway numbering convention.

---

## Database Schema (new migrations)

### `V???__pool_indexer_uniswap_v3.sql`

```sql
-- Tracks the highest finalized block fully processed per chain+contract
CREATE TABLE pool_indexer_checkpoints (
    chain_id       BIGINT NOT NULL,
    contract       BYTEA  NOT NULL,      -- factory or pool address
    block_number   BIGINT NOT NULL,
    PRIMARY KEY (chain_id, contract)
);

-- One row per discovered pool (from PoolCreated events on the factory)
CREATE TABLE uniswap_v3_pools (
    chain_id       BIGINT  NOT NULL,
    address        BYTEA   NOT NULL,     -- pool address
    token0         BYTEA   NOT NULL,
    token1         BYTEA   NOT NULL,
    fee            INT     NOT NULL,     -- fee tier in bps (500, 3000, 10000)
    created_block  BIGINT  NOT NULL,
    PRIMARY KEY (chain_id, address)
);

-- Current state of each pool (updated on every Swap/Mint/Burn that changes it)
CREATE TABLE uniswap_v3_pool_states (
    chain_id        BIGINT  NOT NULL,
    pool_address    BYTEA   NOT NULL,
    block_number    BIGINT  NOT NULL,
    sqrt_price_x96  BYTEA   NOT NULL,   -- U256 as 32 bytes big-endian
    liquidity       BYTEA   NOT NULL,   -- U256 as 32 bytes big-endian
    tick            INT     NOT NULL,
    PRIMARY KEY (chain_id, pool_address),
    FOREIGN KEY (chain_id, pool_address) REFERENCES uniswap_v3_pools(chain_id, address)
);

-- Active ticks per pool (rows with liquidityNet = 0 are pruned)
CREATE TABLE uniswap_v3_ticks (
    chain_id        BIGINT  NOT NULL,
    pool_address    BYTEA   NOT NULL,
    tick_idx        INT     NOT NULL,
    liquidity_net   BYTEA   NOT NULL,   -- i128 as 16 bytes big-endian (signed)
    PRIMARY KEY (chain_id, pool_address, tick_idx),
    FOREIGN KEY (chain_id, pool_address) REFERENCES uniswap_v3_pools(chain_id, address)
);

CREATE INDEX ON uniswap_v3_ticks (chain_id, pool_address);
```

---

## Indexer Logic (`src/indexer/uniswap_v3.rs`)

Since we only care about finalized blocks there's no reorg handling — simplified compared to existing `EventHandler`.

```
loop:
  finalized_block = eth_getBlockByNumber("finalized")
  last_indexed    = db::get_checkpoint(factory_address)

  if last_indexed >= finalized_block → sleep, continue

  for block_range in chunks(last_indexed+1..=finalized_block, CHUNK_SIZE=500):
    logs = eth_getLogs(filter{
      from_block: range.start,
      to_block:   range.end,
      topics: [PoolCreated, Initialize, Mint, Burn, Swap]
      // No address filter (perf: same reason as existing event_fetching.rs)
    })
    db::apply_logs(tx, logs)         // single DB transaction per chunk
    db::set_checkpoint(tx, block_range.end)
    commit tx
```

### Events consumed

All from `UniswapV3Pool` ABI + `IUniswapV3Factory` ABI (already in `crates/contracts/`):

| Source | Event | DB effect |
|---|---|---|
| Factory | `PoolCreated(token0, token1, fee, tickSpacing, pool)` | INSERT into `uniswap_v3_pools` |
| Pool | `Initialize(sqrtPriceX96, tick)` | INSERT/UPDATE `uniswap_v3_pool_states` |
| Pool | `Mint(sender, owner, tickLower, tickUpper, amount, ...)` | +amount to tick_lower liquidityNet, -amount from tick_upper |
| Pool | `Burn(owner, tickLower, tickUpper, amount, ...)` | -amount from tick_lower, +amount to tick_upper |
| Pool | `Swap(..., sqrtPriceX96, liquidity, tick)` | UPDATE pool state row |

Rows with `liquidity_net = 0` in `uniswap_v3_ticks` are deleted (pruned on write).

We do NOT need: `Collect`, `CollectProtocol`, `Flash`, `SetFeeProtocol`, `IncreaseObservationCardinalityNext`.

---

## REST API (`src/api/uniswap_v3.rs`)

Base path: `/api/v1`

### `GET /api/v1/uniswap/v3/latest-block`

Returns the latest fully-indexed finalized block. Replaces `_meta { block { number } }` subgraph query.

```json
{ "block_number": 21800000 }
```

### `GET /api/v1/uniswap/v3/pools`

Returns all known pools with current state. Cursor-based pagination to match the existing `SubgraphClient.paginated_query` behaviour.

Query params:
- `block` (required) — query state at this block number
- `after` (optional) — cursor: last seen pool address (hex)
- `limit` (optional, default 1000)

Response:
```json
{
  "pools": [
    {
      "id": "0x...",
      "token0": { "id": "0x...", "decimals": 18 },
      "token1": { "id": "0x...", "decimals": 6 },
      "fee_tier": "3000",
      "liquidity": "12345678",
      "sqrt_price": "1234567890",
      "tick": -887272,
      "ticks": null
    }
  ],
  "next_cursor": "0x..." // null if last page
}
```

### `GET /api/v1/uniswap/v3/pools/{pool_address}/ticks`

Query params:
- `block` (required)

Response:
```json
{
  "pool": "0x...",
  "ticks": [
    { "tick_idx": -887272, "liquidity_net": "1000000" },
    ...
  ]
}
```

This is called in batch by the driver (chunked by `max_pools_per_tick_query`). Having per-pool tick endpoint keeps it simple and avoids a multi-pool batch endpoint for now.

### `GET /health`

Standard liveness check.

---

## Configuration (`src/config.rs`)

```toml
[database]
url = "postgresql://..."
max-connections = 10

[indexer]
chain-id = 1
rpc-url = "https://..."
factory-address = "0x1F98431c8aD98523631AE4a59f267346ea31F984"  # UniV3 mainnet factory
chunk-size = 500          # blocks per eth_getLogs call
poll-interval-secs = 3   # how often to poll for new finalized block

[api]
bind-address = "0.0.0.0:7777"
```

---

## Critical Files

| File | Role |
|---|---|
| `crates/contracts/artifacts/UniswapV3Pool.json` | ABI source for event decoding |
| `crates/contracts/artifacts/IUniswapV3Factory.json` | ABI source for PoolCreated event |
| `database/sql/V???__pool_indexer.sql` | New migrations |
| `crates/pool-indexer/` | New crate (lib + binary) |
| `Cargo.toml` (workspace) | Add new crate to workspace members |

---

## Verification

1. **Unit tests** for DB query functions (using test transactions that rollback).
2. **Integration test**: spin up the indexer against an anvil fork (`FORK_URL_MAINNET`), index a small block range (e.g. 100 blocks), assert known pool addresses appear in DB with correct state.
3. **API test**: call `GET /pools`, `GET /pools/{addr}/ticks`, and `GET /latest-block` against a running indexer, verify responses match known on-chain state.
4. `cargo clippy` + `cargo +nightly fmt --all` before PR.
