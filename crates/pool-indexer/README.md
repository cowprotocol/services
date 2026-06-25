# pool-indexer

Indexes Uniswap V3 pools, ticks, and pool state into Postgres and serves
the data over HTTP. The driver consumes this API in place of a
third-party Uniswap V3 subgraph.

For each (chain, factory) pair, the indexer seeds its DB from a subgraph
at a fixed block, catches up to the chain tip via RPC events, then stays
live by polling new blocks. Drivers consume it via the `pool-indexer-url`
field in their Uniswap V3 liquidity config.

## Bootstrap and serve

Startup has two phases:

- **Bootstrap** — initial subgraph seed plus catch-up to the finalized head.
  One-time and slow (minutes on a large chain).
- **Serve** — live block polling and the HTTP API. No long startup cost.

`pool-indexer --config <toml>` runs both in one process: it bootstraps when the
DB has no checkpoint, then serves. This is the single-container deployment.

`pool-indexer --bootstrap-only true --config <toml>` runs only the bootstrap
phase and then exits 0, binding no HTTP ports. It is **idempotent**: on a DB
that already has a checkpoint it skips the seed and catch-up entirely (never
touching the subgraph) and returns immediately, so re-running it is a fast, safe
no-op.

Running bootstrap as a separate step ahead of serving keeps serve startup fast:
the serve process finds the checkpoint already present and flips `/startup` ready
almost immediately.

## Running locally

Create `crates/pool-indexer/config.local.toml` first (schema = the
`Configuration` struct in `src/config.rs`): a `[database]` section, one
`[network]` block with a single factory, and optional `[api]` / `[metrics]`
sections. String fields accept `%ENV_VAR` so secrets can come from the
environment instead of being written into the file.

The indexer uses its own database, migrated from `database/sql-pool-indexer`
(separate from the shared autopilot/orderbook set in `database/sql`). From the
repository root:

```bash
docker compose up -d db
# create + migrate the indexer's own database via flyway (database/sql-pool-indexer)
docker compose up migrations-pool-indexer

# one process for both phases:
cargo run --release -p pool-indexer -- --config crates/pool-indexer/config.local.toml

# or split bootstrap from serve:
cargo run --release -p pool-indexer -- --bootstrap-only true --config crates/pool-indexer/config.local.toml
cargo run --release -p pool-indexer -- --config crates/pool-indexer/config.local.toml
```
