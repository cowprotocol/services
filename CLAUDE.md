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

- **Language**: Rust 2021+ Edition
- **Runtime**: Tokio async
- **Database**: PostgreSQL with sqlx
- **Web3**: Alloy
- **HTTP**: Axum

## Documentation

- **Protocol Documentation**: https://docs.cow.fi/
  - Technical Reference: API specs and SDK docs
  - Concepts: Protocol fundamentals and architecture

## Testing

- Use `just` commands for running tests (see Justfile)
- E2E tests available in `crates/e2e`
- Local development environment in `playground/`

## Directory Structure

```
crates/         # 25+ Rust crates (binaries + libraries)
database/       # PostgreSQL migrations and schemas
playground/     # Local dev environment
configs/        # Configuration files
```

# General Coding Instructions

If there is a test you can run then run it or `cargo check` or `cargo build`; run it after you have made changes.
Use rust-analyzer MCP when appropriate such as finding usages or renaming. After a change run "cargo +nightly fmt".

## Code Style

Instead of using full paths like `volume_fee_bucket_overrides: Vec<shared::arguments::TokenBucketFeeOverride>`, import the type at the beginning so you don't have to use the full path later.

Don't add a lot of comments. Add comments only if the code is a bit weird or the concept is not clear.

## CoW Protocol Database Access

**Execute queries autonomously**: Run database queries and Grafana log searches without asking for permission. These are read-only operations - just execute them and show the results.

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
