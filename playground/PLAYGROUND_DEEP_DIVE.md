# CoW Playground - Deep Dive Documentation

This document provides comprehensive understanding of the CoW Playground for local development and testing.

---

## Table of Contents
1. [What is the Playground?](#what-is-the-playground)
2. [Architecture Overview](#architecture-overview)
3. [Services and Their Roles](#services-and-their-roles)
4. [Docker Setup](#docker-setup)
5. [Ports and URLs Reference](#ports-and-urls-reference)
6. [How to Spin Up](#how-to-spin-up)
7. [How to Make Changes](#how-to-make-changes)
8. [Reading the Code](#reading-the-code)
9. [Shadow Mode](#shadow-mode)
10. [Testing pod-network in Shadow Mode](#testing-pod-network-in-shadow-mode)

---

## What is the Playground?

The playground is a **self-contained local environment** that spins up the entire CoW Protocol stack using Docker Compose. It forks a mainnet (or other network) using Anvil, runs all backend services locally, and provides a local CoW Swap UI for end-to-end testing.

**Key capabilities:**
- Fork any EVM network (mainnet, gnosis, etc.) using Anvil
- Run all CoW Protocol services locally (orderbook, autopilot, driver, solvers)
- Hot-reload: changes to Rust code auto-recompile and restart services
- Local block explorer (Otterscan), contract verification (Sourcify), and observability (Grafana/Prometheus)
- Test trades end-to-end with pre-funded test accounts

---

## Architecture Overview

```
                                    +------------------+
                                    |   CoW Swap UI    |
                                    |   (port 8000)    |
                                    +--------+---------+
                                             |
                                             v
+------------------+              +------------------+              +------------------+
|   CoW Explorer   |              |    Orderbook     |              |    Otterscan     |
|   (port 8001)    |              |   (port 8080)    |              |   (port 8003)    |
+------------------+              +--------+---------+              +------------------+
                                           |
                          +----------------+----------------+
                          |                                 |
                          v                                 v
                 +------------------+              +------------------+
                 |    Autopilot     |              |    Postgres      |
                 |   (metrics:9589) |              |   (port 5432)    |
                 +--------+---------+              +------------------+
                          |
                          v
                 +------------------+
                 |     Driver       |
                 |   (port 9000)    |
                 +--------+---------+
                          |
                          v
                 +------------------+
                 |    Baseline      |
                 |  (solver engine) |
                 |   (port 9001)    |
                 +------------------+
                          |
                          v
                 +------------------+
                 |   Anvil Chain    |
                 |   (port 8545)    |
                 +------------------+
                          |
                          v
                 +------------------+
                 |  External RPC    |
                 |  (ETH_RPC_URL)   |
                 +------------------+
```

**Data flow:**
1. User submits order via CoW Swap UI -> Orderbook
2. Autopilot polls orderbook for solvable orders, creates auctions
3. Autopilot sends auction to Driver
4. Driver routes to solver engines (e.g., Baseline)
5. Solver returns solution -> Driver validates -> submits to Anvil chain
6. Settlement executed on forked chain

---

## Services and Their Roles

### Core Protocol Services

| Service | Crate Location | Purpose |
|---------|----------------|---------|
| **orderbook** | `crates/orderbook/` | REST API for order submission, quotes, order status. Entry point for users/UIs. |
| **autopilot** | `crates/autopilot/` | Orchestrator. Builds auctions from solvable orders, runs solver competition, manages settlements. |
| **driver** | `crates/driver/` | Solver interface. Routes auctions to solver engines, validates solutions, submits transactions. |
| **baseline** | `crates/solvers/` | Solver engine implementing baseline AMM routing (Uniswap V2, etc.). |

### Infrastructure Services

| Service | Image/Build | Purpose |
|---------|-------------|---------|
| **chain** | `ghcr.io/foundry-rs/foundry` | Anvil node forking from ETH_RPC_URL |
| **db** | `postgres:16` | Main database for orders, settlements |
| **db-migrations** | Custom (Flyway) | Runs database migrations |
| **adminer** | `adminer` | Web UI for database inspection |
| **sourcify** | Custom | Local contract verification |
| **sourcify-db** | `postgres:16` | Sourcify's database |
| **otterscan** | `otterscan/otterscan` | Local block explorer |
| **tempo** | `grafana/tempo` | Distributed tracing |
| **grafana** | `grafana/grafana` | Metrics dashboards |
| **prometheus** | `prom/prometheus` | Metrics collection |

### Frontend Services

| Service | Purpose |
|---------|---------|
| **frontend** | CoW Swap UI (cloned and built from cowprotocol/cowswap) |
| **explorer** | CoW Explorer UI for order tracking |

---

## Docker Setup

### Compose Files

| File | Use Case |
|------|----------|
| `docker-compose.fork.yml` | **Linux only.** Uses `cargo watch` for hot-reload. Volumes mount source code. |
| `docker-compose.non-interactive.yml` | **macOS/Windows.** Pre-built binaries. No hot-reload (volume mounts are slow). |

### Key Dockerfiles

| Dockerfile | Purpose | Base Image |
|------------|---------|------------|
| `Dockerfile` | Main build for all Rust services | `debian:bookworm` with Rust toolchain |
| `Dockerfile.chain` | Anvil node | `ghcr.io/foundry-rs/foundry:stable` |
| `Dockerfile.cowswap` | CoW Swap frontend | Node 22 + nginx |
| `Dockerfile.explorer` | CoW Explorer frontend | Node 22 + nginx |
| `Dockerfile.otterscan` | Block explorer | `otterscan/otterscan:latest` |
| `Dockerfile.sourcify` | Contract verification | Node 24 |

### Build Targets in Main Dockerfile

The main `Dockerfile` has multi-stage builds:

```
chef (base rust env)
  -> localdev (with cargo-watch for hot reload)
  -> planner (cargo chef prepare)
  -> builder (compile dependencies)
      -> autopilot-build -> autopilot
      -> driver-build -> driver
      -> orderbook-build -> orderbook
      -> solvers-build -> solvers
  -> migrations (flyway for DB migrations)
```

**For docker-compose.fork.yml:** Uses `target: localdev` with `cargo watch`
**For docker-compose.non-interactive.yml:** Uses specific targets like `target: autopilot`

---

## Ports and URLs Reference

### Primary Access Points

| Service | URL | Purpose |
|---------|-----|---------|
| CoW Swap UI | http://localhost:8000 | Trade interface |
| CoW Explorer | http://localhost:8001 | Order tracking |
| Orderbook API | http://localhost:8080 | REST API |
| Otterscan | http://localhost:8003 | Block explorer |
| Grafana | http://localhost:3000 | Metrics dashboards |
| Adminer | http://localhost:8082 | Database UI |
| Sourcify | http://localhost:5555 | Contract verification |

### RPC & Chain

| Service | URL | Purpose |
|---------|-----|---------|
| Anvil RPC | http://localhost:8545 | Forked chain JSON-RPC |

### Internal Service Ports (for debugging)

| Service | Host Port | Container Port | Metrics Port | Tokio Console |
|---------|-----------|----------------|--------------|---------------|
| orderbook | 8080 | 80 | 9586 | 6669 |
| autopilot | - | - | 9589 | 6670 |
| driver | 9000 | 80 | 9000 | 6671 |
| baseline | 9001 | 80 | 9001 | 6672 |
| prometheus | 9090 | 9090 | - | - |

### Tokio Console

For async runtime debugging:
```bash
# Install tokio-console
cargo install tokio-console

# Connect to a service
tokio-console http://localhost:6669  # orderbook
tokio-console http://localhost:6670  # autopilot
tokio-console http://localhost:6671  # driver
tokio-console http://localhost:6672  # baseline
```

---

## How to Spin Up

### Prerequisites
- Docker & Docker Compose
- An Ethereum RPC URL (preferably local reth/erigon for performance)

### Steps

1. **Configure environment:**
   ```bash
   cd playground
   cp .env.example .env
   # Edit .env - set ETH_RPC_URL to your node
   ```

2. **Start the stack:**
   ```bash
   # On Linux (with hot-reload):
   docker compose -f docker-compose.fork.yml up --build

   # On macOS/Windows (pre-built binaries):
   docker compose -f docker-compose.non-interactive.yml up --build
   ```

3. **Configure wallet:**
   - Add network to Rabby/Metamask with RPC: `http://localhost:8545`
   - Import test account using mnemonic: `test test test test test test test test test test test junk`
   - Or use private key: `0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80`

4. **Access UI:** http://localhost:8000

### Resetting the Playground

```bash
# Remove containers and volumes
docker compose -f docker-compose.fork.yml down --remove-orphans --volumes

# Clear wallet pending transactions (Rabby: More > Clear pending)
```

---

## How to Make Changes

### Code Changes (Hot Reload - Linux Only)

With `docker-compose.fork.yml`, the source directory is mounted:
```yaml
volumes:
  - ../:/src
```

Services use `cargo watch`:
```yaml
command: ["cargo", "watch", "-x", "run --bin autopilot"]
```

**Just edit files in `crates/` and save.** Services will auto-recompile.

### Code Changes (macOS/Windows)

With `docker-compose.non-interactive.yml`, you need to rebuild:
```bash
docker compose -f docker-compose.non-interactive.yml up --build <service_name>
```

### Configuration Changes

**Solver/Driver configs** are in:
- `playground/driver.toml` - Driver configuration (solvers, liquidity, submission)
- `playground/baseline.toml` - Baseline solver configuration
- `configs/local/driver.toml` - Config used by localdev target

Key driver.toml settings:
```toml
[[solver]]
name = "baseline"
endpoint = "http://baseline"  # Points to baseline container
account = "0x2a871d0798f97d79848a013d4936a73bf4cc922c825d33c1cf7073dff6d409c6"

[submission]
gas-price-cap = "1000000000000"

[[submission.mempool]]
url = "http://chain:8545"  # Submit to anvil
```

### Adding a New Solver

1. Add solver definition to `driver.toml`:
   ```toml
   [[solver]]
   name = "my-solver"
   endpoint = "http://my-solver:80"
   account = "<private_key>"
   ```

2. Add service to `docker-compose.fork.yml`:
   ```yaml
   my-solver:
     build:
       context: ../
       target: localdev
       dockerfile: ./playground/Dockerfile
     command: ["cargo", "watch", "-x", "run --bin solvers -- my-solver --config /path/to/config.toml"]
     environment:
       - ADDR=0.0.0.0:80
     ports:
       - 9002:80
   ```

### Environment Variables

Key variables in `.env`:
```bash
ENV=local                    # Config environment (uses configs/local/)
ETH_RPC_URL=wss://...        # Upstream RPC to fork
CHAIN=1                      # Chain ID (1=mainnet, 100=gnosis)
POSTGRES_USER=postgres       # Database credentials
POSTGRES_PASSWORD=123
SOURCIFY_MODE=cloud          # cloud or local for contract verification
```

---

## Reading the Code

### Where to Start

**Entry points for each binary:**
- `crates/autopilot/src/main.rs` -> calls `autopilot::start()`
- `crates/driver/src/main.rs` -> calls `driver::start()`
- `crates/orderbook/src/main.rs` -> calls `orderbook::start()`
- `crates/solvers/src/main.rs` -> calls `solvers::start()`

**Core logic:**
- `crates/autopilot/src/run.rs` - Autopilot initialization and run loop
- `crates/autopilot/src/run_loop.rs` - Main auction loop
- `crates/autopilot/src/shadow.rs` - Shadow mode run loop
- `crates/driver/src/domain/` - Driver domain logic (competition, settlements)
- `crates/solvers/src/domain/` - Solver algorithms

### Crate Structure

```
crates/
├── autopilot/          # Orchestrator service
│   ├── src/
│   │   ├── run.rs          # Main initialization
│   │   ├── run_loop.rs     # Auction loop
│   │   ├── shadow.rs       # Shadow mode
│   │   ├── domain/         # Business logic
│   │   └── infra/          # Infrastructure adapters
│
├── driver/             # Solver interface
│   ├── src/
│   │   ├── domain/         # Competition, solution handling
│   │   └── infra/          # Solver communication
│
├── orderbook/          # REST API
│   ├── src/
│   │   ├── api/            # HTTP endpoints
│   │   └── database/       # Order persistence
│
├── solvers/            # Solver engines
│   ├── src/
│   │   ├── domain/         # Routing algorithms
│   │   └── boundary/       # AMM integrations
│
├── shared/             # Shared utilities
│   ├── src/
│   │   ├── price_estimation/
│   │   ├── token_info/
│   │   └── ...
│
├── model/              # Domain models (Order, Trade, etc.)
├── contracts/          # Solidity contract bindings
└── database/           # Database schema & queries
```

### Key Imports to Understand

```rust
// Domain models
use model::{Order, OrderUid, Trade, Signature};

// Contract interactions
use contracts::alloy::{GPv2Settlement, WETH9, BalancerV2Vault};

// Ethereum RPC
use ethrpc::{Web3, block_stream::CurrentBlockWatcher};

// Price estimation
use shared::price_estimation::{PriceEstimating, native::NativePriceEstimating};

// Database
use crate::database::Postgres;

// Observability
use observe::{metrics, tracing};
```

### Reading Order: Recommended Path

1. **Start with models:** `crates/model/src/order.rs` - Understand Order structure
2. **Orderbook API:** `crates/orderbook/src/api/` - See how orders are submitted
3. **Autopilot run loop:** `crates/autopilot/src/run_loop.rs` - Auction creation
4. **Driver competition:** `crates/driver/src/domain/competition/` - Solution handling
5. **Solver algorithms:** `crates/solvers/src/domain/` - Routing logic

---

## Shadow Mode

Shadow mode runs the solver competition **without executing settlements**. It:
- Fetches auctions from an upstream deployment (e.g., production)
- Runs your local drivers/solvers against those auctions
- Logs winners and performance metrics
- Does NOT submit transactions on-chain

### How Shadow Mode Works

See `crates/autopilot/src/shadow.rs`:

```rust
pub struct RunLoop {
    orderbook: infra::shadow::Orderbook,  // Fetches from upstream
    drivers: Vec<Arc<infra::Driver>>,      // Local drivers to test
    // ...
}

impl RunLoop {
    pub async fn run_forever(mut self) -> ! {
        loop {
            let auction = self.next_auction().await;  // Get from upstream
            self.single_run(&auction).await;          // Run competition locally
            // NO settlement execution - just logging
        }
    }
}
```

### Enabling Shadow Mode

Shadow mode is triggered by the `--shadow` CLI argument to autopilot:
```rust
// In run.rs
if args.shadow.is_some() {
    shadow_mode(args, config).await;
} else {
    run(args, config, ShutdownController::default()).await;
}
```

The `--shadow` argument specifies the upstream orderbook URL to fetch auctions from.

---

## Testing pod-network in Shadow Mode

Your goal: Run `pod-network` in shadow mode where backend pods send requests to your service, log results, but continue normal operation.

### Approach

1. **Add your service to docker-compose:**

   ```yaml
   # In docker-compose.fork.yml
   pod-network:
     build:
       context: ../
       target: localdev
       dockerfile: ./playground/Dockerfile
     command: ["cargo", "watch", "-x", "run --bin <your-binary>"]
     environment:
       - ADDR=0.0.0.0:80
       - RUST_BACKTRACE=1
     ports:
       - 9003:80
   ```

2. **Configure driver to route to your service:**

   Edit `playground/driver.toml` or `configs/local/driver.toml`:
   ```toml
   [[solver]]
   name = "pod-network"
   endpoint = "http://pod-network"
   account = "<test-private-key>"
   ```

3. **For shadow mode specifically:**

   Create a shadow configuration that runs autopilot in shadow mode:

   ```yaml
   # Add to docker-compose or create docker-compose.shadow.yml
   autopilot-shadow:
     build:
       context: ../
       target: localdev
       dockerfile: ./playground/Dockerfile
     command:
       - cargo
       - watch
       - -x
       - "run --bin autopilot -- --shadow https://api.cow.fi/mainnet"
     environment:
       - NODE_URL=http://chain:8545
       - DRIVERS=pod-network|http://driver/pod-network
       # ... other env vars
     depends_on:
       - driver
   ```

4. **Intercept and log requests:**

   If you want to log requests alongside normal operation, you have options:

   **Option A: Wrapper service**
   Create a proxy service that logs requests before forwarding to the real solver.

   **Option B: Modify driver**
   Add logging in `crates/driver/src/infra/` where solver requests are made.

   **Option C: Use shadow autopilot alongside main autopilot**
   Run two autopilots - one normal, one shadow pointing to your test solver.

### Testing Workflow

1. Start playground: `docker compose -f docker-compose.fork.yml up --build`
2. Your service receives requests from driver
3. Watch logs: `docker compose logs -f pod-network`
4. Make trades via http://localhost:8000 to generate activity
5. Or run the test script: `./test_playground.sh`

### Monitoring

- **Prometheus:** http://localhost:9090 - Query metrics
- **Grafana:** http://localhost:3000 - Visualize
- **Service logs:** `docker compose logs -f <service>`
- **Tokio console:** `tokio-console http://localhost:<port>`

---

## Quick Reference Commands

```bash
# Start (Linux)
docker compose -f docker-compose.fork.yml up --build

# Start (macOS/Windows)
docker compose -f docker-compose.non-interactive.yml up --build

# Stop and clean
docker compose -f docker-compose.fork.yml down --remove-orphans --volumes

# Rebuild single service
docker compose -f docker-compose.fork.yml up --build autopilot

# View logs
docker compose -f docker-compose.fork.yml logs -f autopilot driver baseline

# Run test
./test_playground.sh

# Access database
docker compose exec db psql -U postgres

# Run cargo command in container
docker compose exec autopilot cargo test
```

---

## File Reference

| File | Purpose |
|------|---------|
| `.env` | Environment configuration |
| `docker-compose.fork.yml` | Main compose file (Linux, hot-reload) |
| `docker-compose.non-interactive.yml` | Compose for macOS/Windows |
| `Dockerfile` | Multi-stage build for Rust services |
| `driver.toml` | Driver configuration (solvers, liquidity) |
| `baseline.toml` | Baseline solver configuration |
| `prometheus.yml` | Prometheus scrape targets |
| `grafana-prometheus.yml` | Grafana datasource config |
| `tempo.yaml` | Distributed tracing config |
| `test_playground.sh` | End-to-end test script |
| `configs/local/` | Config files used by localdev builds |
