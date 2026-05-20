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

`run-local.sh` is a one-shot script that resets the local stack and starts
the indexer from scratch. In order, it:

1. Tears down the docker compose stack and deletes the postgres volume
   (`docker compose down --volumes`), so any previous DB state is gone.
2. Brings the `db` service back up and waits for postgres to accept
   connections.
3. Applies all Flyway migrations from `database/sql/`
   (`docker compose run --rm migrations`).
4. Runs `cargo run --release -p pool-indexer -- --config crates/pool-indexer/config.local.toml`.

You do not need to start docker compose or run migrations beforehand — the
script does both. It wipes the local database volume on every run, so use it
on development machines only.

```bash
./crates/pool-indexer/run-local.sh
```

Before running it, create `crates/pool-indexer/config.local.toml`. The schema
is the `Configuration` struct in `src/config.rs`: a `[database]` section, one
or more `[[network]]` blocks (each with a single factory), and optional
`[api]` and `[metrics]` sections. String fields accept `%ENV_VAR`, so RPC
URLs and subgraph bearer tokens can be sourced from the environment instead
of being written into the file.
