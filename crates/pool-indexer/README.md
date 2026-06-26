# pool-indexer

Indexes Uniswap V3 pools, ticks, and pool state into Postgres and serves
the data over HTTP. The driver consumes this API in place of a
third-party Uniswap V3 subgraph.

For each (chain, factory) pair, the indexer seeds its DB from a subgraph
at a fixed block, catches up to the chain tip via RPC events, then stays
live by polling new blocks. Drivers consume it via the `pool-indexer-url`
field in their Uniswap V3 liquidity config.

## Running locally

Create `crates/pool-indexer/config.local.toml` first (schema = the
`Configuration` struct in `src/config.rs`): a `[database]` section, one
`[network]` block with a single factory, and optional `[api]` / `[metrics]`
sections. String fields accept `%ENV_VAR` so secrets can come from the
environment instead of being written into the file.

Then, from the repository root, reset the local stack and start the indexer:

```bash
# wipes the local DB — dev machines only
docker compose down --volumes
docker compose up -d db
docker compose run --rm migrations
cargo run --release -p pool-indexer -- --config crates/pool-indexer/config.local.toml
```
