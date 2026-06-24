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

`pool-indexer --bootstrap-only --config <toml>` runs only the bootstrap phase
and then exits 0, binding no HTTP ports. It is **idempotent**: on a DB that
already has a checkpoint it skips the seed and catch-up entirely (never touching
the subgraph) and returns immediately, so re-running it — for example a
restarted bootstrap initContainer — is a fast, safe no-op.

This lets K8s run bootstrap as an initContainer and apply a tight startupProbe
to the serve container, which finds the checkpoint already present and flips
`/startup` ready almost immediately:

```yaml
initContainers:
  - name: init-db      # flyway, schema from the indexer's own location
    command: ["flyway", "-locations=filesystem:/flyway/sql-pool-indexer", "migrate"]
  - name: bootstrap    # one-time seed + catch-up; idempotent on restart
    command: ["pool-indexer", "--bootstrap-only", "--config", "/etc/config/pool-indexer.toml"]
containers:
  - name: pool-indexer # serve; DB already seeded, so startup is fast
    command: ["pool-indexer", "--config", "/etc/config/pool-indexer.toml"]
```

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
# apply the indexer schema (database/sql-pool-indexer) to the indexer's DB
psql "$POOL_INDEXER_DB_URL" -f database/sql-pool-indexer/V110__pool_indexer_uniswap_v3.sql

# one process for both phases:
cargo run --release -p pool-indexer -- --config crates/pool-indexer/config.local.toml

# or split bootstrap from serve, as the K8s deployment does:
cargo run --release -p pool-indexer -- --bootstrap-only --config crates/pool-indexer/config.local.toml
cargo run --release -p pool-indexer -- --config crates/pool-indexer/config.local.toml
```
