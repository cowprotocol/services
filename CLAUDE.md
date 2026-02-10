# Cow Protocol Services

Backend services for Cow Protocol, a decentralized trading protocol with batch auctions on EVM networks.

## Project Structure

This is a Rust workspace containing multiple services and libraries:

### Main Services (Binaries)
- **orderbook** - HTTP API for order submission and queries
- **autopilot** - Protocol driver that manages auctions
- **driver** - Handles liquidity collection and solution selection
- **solvers** - Internal solver engine (baseline)
- **refunder** - Handles refunds

### Key Libraries
- **shared** - Common functionality (pricing, liquidity, gas estimation)
- **database** - PostgreSQL abstraction and migrations
- **model** - Serialization models for API
- **contracts** - Smart contract bindings
- **ethrpc** - Extended Ethereum RPC client with batching layer
- **chain** - Blockchain interaction utilities
- **number** - Numerical type extensions and conversions for 256-bit integers
- **app-data** - Order metadata validation with 8KB default size limit
- **alerter** - Monitors orderbook metrics for orders that should be solved but aren't
- **testlib** - Shared helpers for writing unit and end-to-end tests
- **observe** - Initialization and helper functions for logging and metrics

## Architecture Overview

```
User signs order → Orderbook validates → Autopilot includes in auction
                                              ↓
                    ┌─────────────────────────┴─────────────────────────┐
                    ↓                                                   ↓
          Colocated External Solvers                    Our Drivers + Non-Colocated Solvers
          (run their own driver+solver)                      ↓                    ↓
                    │                                 Our solvers          External solver APIs
                    │                                 (baseline,           (non-colocated partners
                    │                                  balancer, ...)       like 1inch, 0x, etc)
                    └─────────────────────────┬─────────────────────────┘
                                              ↓
                              Autopilot ranks solutions, picks winner(s)
                                              ↓
                              Winning driver submits to chain (2-3 block window)
                                              ↓
                              Settlement contract executes:
                              1. Pre-interactions (incl user pre-hooks)
                              2. Transfer sell tokens in
                              3. Main interactions (swaps/routing)
                              4. Pay out buy tokens
                              5. Post-interactions (incl user post-hooks)
                                              ↓
                              Circuit breaker monitors compliance
```

**Solver types:**
- **Colocated**: External partners run their own driver + solver. Full control, full responsibility.
- **Non-colocated**: We run the driver, configured with their solver API endpoint. We handle simulation/submission.

**Key components:**
- **Orderbook**: Validates + stores orders, handles quoting
- **Autopilot**: Central auctioneer, runs every ~12-15s (eventually every block), filters orders, adds fee policies, sends auction to solvers, ranks solutions
- **Driver**: Fetches liquidity, encodes solutions to calldata, simulates, submits to chain. Handles everything except route-finding.
- **Solver Engine**: Pure math — finds best routes/matches. Can be internal (baseline, balancer) or external API calls.
- **Circuit Breaker**: Monitors on-chain settlements match off-chain auction outcomes. Jails misbehaving solvers.

## Technology Stack

- **Language**: Rust 2024 Edition
- **Runtime**: Tokio async
- **Database**: PostgreSQL with sqlx
- **Web3**: Alloy
- **HTTP**: Axum

## Documentation

- **Protocol Documentation**: https://docs.cow.fi/
  - Technical Reference: API specs and SDK docs
  - Concepts: Protocol fundamentals and architecture
- **Alloy (Web3 library)**: Fetch https://alloy.rs/introduction/prompting for an AI-optimized guide covering providers, transactions, contracts, and migration from ethers-rs

## Development Commands

### Testing
- Use `cargo nextest run` instead of `cargo test` (CI uses nextest and handles global state differently)
- Run specific test suites:
  - Unit tests: `cargo nextest run`
  - Database tests: `cargo nextest run postgres -p orderbook -p database -p autopilot --test-threads 1 --run-ignored ignored-only`
  - E2E local tests: `cargo nextest run -p e2e local_node --test-threads 1 --failure-output final --run-ignored ignored-only`
  - E2E forked tests: `cargo nextest run -p e2e forked_node --test-threads 1 --run-ignored ignored-only --failure-output final`
  - Driver tests: `RUST_MIN_STACK=3145728 cargo nextest run -p driver --test-threads 1 --run-ignored ignored-only`
- E2E tests available in `crates/e2e`

### Testing Requirements
- PostgreSQL tests require local database: Run `docker compose up -d` first
- Forked network tests require `anvil` (from Foundry) and RPC URLs
  - Anvil binary: configurable via `ANVIL_COMMAND` env var (defaults to `"anvil"`, must be in PATH)
  - Required env vars: `FORK_URL_MAINNET` and `FORK_URL_GNOSIS` (RPC endpoints for forking)
- Use `--test-threads 1` for database and E2E tests to avoid conflicts
- CI runs doc-tests, unit tests, DB tests, E2E tests (local and forked), and driver tests

### Linting and Formatting
- Format: **always** run with the nightly toolchain: `cargo +nightly fmt --all`
- Spot format: `cargo +nightly fmt -- <path>` (never call stable `cargo fmt`)
- Lint: `cargo clippy --locked --workspace --all-features --all-targets -- -D warnings`
- Check format: `cargo +nightly fmt --all -- --check`

### Local Development Environment
- Start local PostgreSQL: `docker compose up -d`
- Full playground environment: `docker compose -f playground/docker-compose.fork.yml up -d`
- For forked network tests, set environment variables: `FORK_URL_MAINNET` and `FORK_URL_GNOSIS`
- Reset playground: `docker compose -f playground/docker-compose.fork.yml down --remove-orphans --volumes`

## Directory Structure

```
crates/         # 25+ Rust crates (binaries + libraries)
database/       # PostgreSQL migrations and schemas
playground/     # Local dev environment
configs/        # Configuration files
```

## Workspace Configuration

- Rust Edition 2024
- Uses workspace dependencies for consistency
- Tokio-console support: **Only available in playground environment** (set `TOKIO_CONSOLE=true` to activate when running in playground)
- Production builds do **not** include tokio-console overhead
- Runtime log filter changes via UNIX socket at `/tmp/log_filter_override_<program_name>_<pid>.sock`
- Memory allocator: Uses jemalloc by default with built-in heap profiling support (enable at runtime via MALLOC_CONF environment variable). Can optionally use mimalloc via `--features mimalloc-allocator`

## Playground Environment

- Runs in **Fork** mode: anvil forks a real network via `ETH_RPC_URL` (set in `playground/.env`). A clean local network mode is planned but not yet implemented.
- Access full local development stack with CoW Swap UI at http://localhost:8000
- CoW Explorer available at http://localhost:8001
- Orderbook API at http://localhost:8080
- Database admin (Adminer) at http://localhost:8082
- Uses test mnemonic: "test test test test test test test test test test test junk"
- First 10 accounts have 10000 ETH balance by default, set by anvil

## Development Notes

- Binaries support `--help` for comprehensive command documentation
- OpenAPI documentation available for orderbook, driver, and solver APIs
- Performance profiling: Only available in playground (requires tokio-console feature + tokio_unstable cfg)

# General Coding Instructions

If there is a test you can run then run it or `cargo check` or `cargo build`; run it after you have made changes.
Use rust-analyzer MCP when appropriate such as finding usages or renaming. After a change run "cargo +nightly fmt".

## Code Style

Instead of using full paths like `volume_fee_bucket_overrides: Vec<shared::arguments::TokenBucketFeeOverride>`, import the type at the beginning so you don't have to use the full path later.

Don't add a lot of comments. Add comments only if the code is a bit weird or the concept is not clear.

## CoW Protocol Database Access

**Always show the SQL query before executing it** against postgres MCP tools (`mcp__postgres-protocol__query`, `mcp__postgres-analytics__query`).

**Query timeout**: MCP servers are configured with a 120 second timeout. For potentially long-running queries, prefix with `SET statement_timeout = '30s';` (or appropriate duration) to fail fast:
```sql
SET statement_timeout = '30s';
SELECT ... FROM large_table ...;
```
If a query times out, try a different approach (add more filters, use a smaller time range, simplify aggregations, or break into smaller queries).

Read-only replica available via MCP. If that fails for some reason, then you can use psql with:
```bash
source .env.claude && PGPASSWORD="$COW_DB_PASSWORD" psql \
  -h "$COW_DB_HOST" -p "$COW_DB_PORT" -U "$COW_DB_USER" -d <database> -c "<query>"
```
but use MCP where possible.

Databases: `mainnet`, `arbitrum-one`, `base`, `linea`, `polygon`, `xdai`, `sepolia`, `plasma`, `ink`, `bnb` etc.

## RPC Node

Use `$ETH_MAINNET_RPC` from `.env.claude` for mainnet. Use `cast` or whatever tools you want freely.

## Grafana Logs Access

Query logs via the Grafana API (credentials in `.env.claude`):

```bash
source .env.claude && curl -s -H "Authorization: Bearer $GRAFANA_API_TOKEN" \
  "$GRAFANA_URL/api/ds/query" \
  -X POST -H "Content-Type: application/json" \
  -d '{
    "queries": [{
      "refId": "A",
      "datasource": {"type": "victoriametrics-logs-datasource", "uid": "'"$VICTORIA_LOGS_DATASOURCE_UID"'"},
      "expr": "<search_term>",
      "queryType": "instant"
    }],
    "from": "now-1h",
    "to": "now"
  }'
```
Adjust expr for search terms (e.g., plasma, ink, error)
Adjust from/to for time range (e.g., now-15m, now-24h)
Parse log lines with: | jq -r '.results.A.frames[0].data.values[1][]'

## Etherscan API (V2)

Use MCP `mcp__fetch__fetch` tool. API Key in `.env.claude` as `$ETHERSCAN_API_KEY`.

**Important**: V1 API is deprecated. Use V2 with the `chainid` parameter:
- Mainnet: `chainid=1`
- Arbitrum: `chainid=42161`
- Base: `chainid=8453`

Example URL format:
```
https://api.etherscan.io/v2/api?chainid=1&module=account&action=balance&address=<addr>&tag=latest&apikey=<key>
```

Read the API key from `.env.claude` and use it directly in the URL (MCP fetch doesn't do shell variable substitution).

## Investigating orders

When asked to look into what happened to an order read file ./docs/COW_ORDER_DEBUG_SKILL.md and follow the instructions there.
Make heavy use of logs and DB to find all info you need and present finding to the user with evidence.
