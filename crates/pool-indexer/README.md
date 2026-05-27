# pool-indexer

A standalone service that indexes Uniswap V3 pools, ticks, and pool state into
Postgres and serves the data over HTTP. The driver queries this API in place
of a third-party Uniswap V3 subgraph.

For each new (chain, factory) pair, the indexer seeds its database from a
Uniswap V3 subgraph at a fixed block, then catches up to the chain tip by
replaying RPC events, and from then on stays live by polling new blocks. The
driver consumes it via the `pool-indexer-url` field in its Uniswap V3
liquidity config.

## Running locally

Create `crates/pool-indexer/config.local.toml` first. The schema is the
`Configuration` struct in `src/config.rs`: a `[database]` section, one or
more `[[network]]` blocks (each with a single factory), and optional `[api]`
and `[metrics]` sections. String fields accept `%ENV_VAR`, so RPC URLs and
other secrets can be sourced from the environment instead of being written
into the file.

Then, from the repository root, reset the local stack and start the indexer:

```bash
# wipes the local DB — dev machines only
docker compose down --volumes
docker compose up -d db
docker compose run --rm migrations
cargo run --release -p pool-indexer -- --config crates/pool-indexer/config.local.toml
```
