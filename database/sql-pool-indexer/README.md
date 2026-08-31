# Pool-indexer migrations

Flyway migrations for the pool-indexer's own per-network database (e.g.
`ink_pool_indexer`), kept out of the shared `../sql/` set so they don't run
against the autopilot/orderbook main DBs.

The migration image ships both dirs; init containers pick one via `-locations`:

| DB                  | location                                            |
|---------------------|-----------------------------------------------------|
| autopilot/orderbook | `/flyway/sql` (default)                             |
| pool-indexer        | `-locations=filesystem:/flyway/sql-pool-indexer`    |

New pool-indexer migrations go here, never in `../sql/`. `V110` is duplicated
from `../sql/` on purpose: the shared copy can't be deleted (Flyway checksums
applied migrations) so it's cancelled there by `../sql/V111`.

## Schema

The tables below live in the indexer's own per-network database (e.g.
`ink_pool_indexer`), created by the migrations in this directory. `V110` defines
the Uniswap V3 discovery schema and `V111` the Balancer V2 one; a pool-indexer
process indexes whichever protocol(s) its network config enables.

### pool\_indexer\_checkpoints

Highest finalized block processed per `contract_address` (the factory address) by `pool-indexer`. Shared by the Uniswap V3 and Balancer V2 indexers: their factory addresses are distinct contracts so rows never collide, and each protocol's queries filter to its own configured factories. One process per network against its own DB, so there's no `chain_id` column.

 Column             | Type   | Nullable | Details
--------------------|--------|----------|--------
 contract\_address  | bytea  | not null | Factory address (20 bytes)
 block\_number      | bigint | not null |

Indexes:
- PRIMARY KEY: btree (`contract_address`)

### uniswap\_v3\_pools

One row per pool discovered from a `PoolCreated` event. `token{0,1}_{decimals,symbol}` are nullable and filled in by the backfill task. `factory` partitions the table when multiple V3-compatible factories run on the same network so each indexer touches only its own rows.

 Column            | Type     | Nullable | Details
-------------------|----------|----------|--------
 address           | bytea    | not null | Pool address (20 bytes)
 factory           | bytea    | not null | Address of the V3 factory that emitted `PoolCreated`
 token0            | bytea    | not null |
 token1            | bytea    | not null |
 fee               | int      | not null | Hundredths of a basis point (500 = 0.05%, 3000 = 0.3%, 10000 = 1%). `CHECK (fee > 0)`.
 token0\_decimals  | smallint | nullable | `NULL` = not yet fetched. `-1` = sentinel for "fetched but call failed"
 token1\_decimals  | smallint | nullable |
 token0\_symbol    | text     | nullable | `NULL` = not yet fetched. `""` = sentinel for "fetched but call failed"
 token1\_symbol    | text     | nullable |
 created\_block    | bigint   | not null | Block in which the pool was created on-chain

Indexes:
- PRIMARY KEY: btree (`address`)
- Four partial indexes on `(token{0,1})` with predicate `token{0,1}_{symbol,decimals} IS NULL` to power the backfill scan.

### uniswap\_v3\_pool\_states

Current state per pool: `sqrt_price_x96` and `tick` come from the latest `Swap`/`Initialize`; `liquidity` and `block_number` also update on in-range `Mint`/`Burn`. FK → `uniswap_v3_pools`.

**Uniswap V3 pool-state primer.** Three values capture a pool's instantaneous state:

- `sqrt_price_x96` — `sqrt(price) * 2^96` where `price = token1/token0`, stored in Q64.96 fixed-point. The square-root form keeps swap math additive and bounds precision loss over the uint160 range. Mirrors on-chain `slot0.sqrtPriceX96`.
- `tick` — `floor(log_{1.0001}(price))`. Each tick is a ~0.01% price step; the current tick is the bucket the live price falls into. Routers use it to decide which positions are in-range.
- `liquidity` — sum of every position's liquidity whose `tickLower <= current_tick < tickUpper`. This is the `L` in V3's invariant `Δsqrt_price = Δamount / L`. Updates on `Swap` (the event carries the new value) and on `Mint`/`Burn` whose range spans the current tick.

The per-tick deltas that move `liquidity` when the price crosses a tick boundary live in [`uniswap_v3_ticks`](#uniswap_v3_ticks).

 Column            | Type    | Nullable | Details
-------------------|---------|----------|--------
 pool\_address     | bytea   | not null | FK → `uniswap_v3_pools(address)`
 block\_number     | bigint  | not null | Block of the most recent state-changing event (`Swap`, `Initialize`, or in-range `Mint`/`Burn`).
 sqrt\_price\_x96  | numeric | not null | uint160 — see primer above
 liquidity         | numeric | not null | uint128 — see primer above
 tick              | int     | not null | signed int24 — see primer above

Indexes:
- PRIMARY KEY: btree (`pool_address`)

### uniswap\_v3\_ticks

Per-tick liquidity deltas. Rows with `liquidity_net = 0` are pruned. FK → `uniswap_v3_pools`.

**Why deltas instead of per-tick totals.** A V3 position covers `[tickLower, tickUpper)` and contributes to pool liquidity only when the current tick is in that range. We store the entering / exiting deltas at the bounds:

- At `tickLower`: `liquidity_net += position.liquidity` (entering)
- At `tickUpper`: `liquidity_net -= position.liquidity` (exiting)

When a swap crosses a tick boundary, the pool's `liquidity` shifts by `± tick.liquidity_net`. This encoding makes the per-tick aggregate O(1) at swap time — no per-position iteration.

Quoters consult these to predict liquidity changes at tick crossings during swap simulation. Without them, large swaps would be priced as if the liquidity stayed flat, producing wildy wrong quotes

 Column         | Type    | Nullable | Details
----------------|---------|----------|--------
 pool\_address  | bytea   | not null | FK → `uniswap_v3_pools(address)`
 tick\_idx      | int     | not null | Tick coordinate (signed int24); same domain as [`uniswap_v3_pool_states.tick`](#uniswap_v3_pool_states)
 liquidity\_net | numeric | not null | int128, signed — net liquidity entering (+) / exiting (-) at this tick

Indexes:
- PRIMARY KEY: btree (`pool_address`, `tick_idx`)

### balancer\_v2\_pools

One row per pool, discovered from each factory's `PoolCreated` event. `pool_type` is derived from the creating factory (weighted V0 and V3-plus both map to `Weighted`) and stored as the string the API serves. Referenced by `balancer_v2_pool_tokens`. Discovery metadata only — dynamic state (balances, amplification, LBP weights, scaling factors, swap fee) stays on-chain and is fetched by the driver at query time.

 Column         | Type   | Nullable | Details
----------------|--------|----------|--------
 pool\_id       | bytea  | not null | 32-byte Balancer poolId
 address        | bytea  | not null | Pool address (poolId's first 20 bytes)
 factory        | bytea  | not null | Factory that emitted `PoolCreated`
 pool\_type     | text   | not null | `Weighted`, `Stable`, `ComposableStable`, or `LiquidityBootstrapping` (`CHECK`)
 created\_block | bigint | not null | Block the pool was created on-chain

Indexes:
- PRIMARY KEY: btree (`pool_id`)

### balancer\_v2\_pool\_tokens

Tokens per pool in `Vault.getPoolTokens` order. `decimals` is backfilled; `weight` is set only for weighted pools. FK → `balancer_v2_pools`.

 Column    | Type     | Nullable | Details
-----------|----------|----------|--------
 pool\_id  | bytea    | not null | FK → `balancer_v2_pools(pool_id)`
 position  | int      | not null | Index in `getPoolTokens` order
 token     | bytea    | not null | Token address
 decimals  | smallint | nullable | `NULL` = not yet fetched. `-1` = sentinel for "fetched but call failed"
 weight    | numeric  | nullable | Bfp (1e18) normalized weight; weighted pools only, else `NULL`

Indexes:
- PRIMARY KEY: btree (`pool_id`, `position`)
- Partial index on `(token)` with predicate `decimals IS NULL` to power the backfill scan.
