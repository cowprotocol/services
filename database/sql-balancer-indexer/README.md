# Balancer-indexer migrations

Flyway migrations for the Balancer V2 indexer's own per-network database (e.g.
`mainnet_balancer_indexer`), kept out of the shared `../sql/` set so they don't
run against the autopilot/orderbook main DBs. The local/e2e DB is
`balancer_indexer`.

The migration image ships every dir; init containers pick one via `-locations`:

| DB                  | location                                              |
|---------------------|-------------------------------------------------------|
| autopilot/orderbook | `/flyway/sql` (default)                               |
| pool-indexer        | `-locations=filesystem:/flyway/sql-pool-indexer`      |
| balancer-indexer    | `-locations=filesystem:/flyway/sql-balancer-indexer`  |

New Balancer-indexer migrations go here, never in `../sql/`. Unlike the
pool-indexer's `V110` (duplicated from `../sql/` and cancelled there by
`../sql/V111`), these tables are new to this directory, so there's no
shared-set copy to cancel.

## Schema

The tables below live in the indexer's own per-network database, created by the
migrations in this directory. The indexer stores *discovery* metadata only;
dynamic state (balances, amplification, LBP weights, scaling factors, swap fee,
paused) stays on-chain and is fetched by the driver at query time.

### balancer\_v2\_checkpoints

Highest finalized block processed per `factory_address` by the Balancer
indexer. The indexer runs one process per network against its own DB, so
there's no `chain_id` column.

 Column            | Type   | Nullable | Details
-------------------|--------|----------|--------
 factory\_address  | bytea  | not null | Factory address (20 bytes)
 block\_number     | bigint | not null |

Indexes:
- PRIMARY KEY: btree (`factory_address`)

### balancer\_v2\_pools

One row per registered pool, discovered from each factory's `PoolCreated`
event. `pool_type` is derived from which factory created the pool (the
factory→type map comes from config — no on-chain classification call). The
weighted V0 vs V3-plus distinction is recovered driver-side from `factory`, so
it isn't a separate `pool_type`.

 Column          | Type   | Nullable | Details
-----------------|--------|----------|--------
 pool\_id        | bytea  | not null | 32-byte Balancer poolId
 address         | bytea  | not null | Pool address (poolId's first 20 bytes)
 factory         | bytea  | not null | Address of the factory that emitted `PoolCreated`
 pool\_type      | text   | not null | `Weighted` \| `Stable` \| `ComposableStable` \| `LiquidityBootstrapping`. `CHECK`ed; stored as the string the API serves.
 created\_block  | bigint | not null | Block in which the pool was created on-chain

Indexes:
- PRIMARY KEY: btree (`pool_id`)

### balancer\_v2\_pool\_tokens

Tokens per pool, in `Vault.getPoolTokens` order (`position`). `decimals` is
nullable and filled in by the backfill task. `weight` is the Balancer Bfp
(1e18 fixed-point) normalized weight, set only for weighted pools.

 Column     | Type     | Nullable | Details
------------|----------|----------|--------
 pool\_id   | bytea    | not null | FK → `balancer_v2_pools(pool_id)`
 position   | int      | not null | Token index within the pool (`getPoolTokens` order)
 token      | bytea    | not null | Token address (20 bytes)
 decimals   | smallint | nullable | `NULL` = not yet fetched. `-1` = sentinel for "fetched but call failed"
 weight     | numeric  | nullable | Bfp (1e18) normalized weight; `NULL` for stable/composable-stable/LBP

Indexes:
- PRIMARY KEY: btree (`pool_id`, `position`)
- Partial index on `(token)` with predicate `decimals IS NULL` to power the backfill scan.
