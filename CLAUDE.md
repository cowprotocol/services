# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Development Commands

### Testing
- Use `cargo nextest run` instead of `cargo test` (CI uses nextest and handles global state differently)
- Run specific test suites:
  - Unit tests: `cargo nextest run`
  - Database tests: `cargo nextest run postgres -p orderbook -p database -p autopilot --test-threads 1 --run-ignored ignored-only`
  - E2E local tests: `cargo nextest run -p e2e local_node --test-threads 1 --failure-output final --run-ignored ignored-only`
  - E2E forked tests: `cargo nextest run -p e2e forked_node --test-threads 1 --run-ignored ignored-only --failure-output final`
  - Driver tests: `RUST_MIN_STACK=3145728 cargo nextest run -p driver --test-threads 1 --run-ignored ignored-only`

### Linting and Formatting
- Format: `cargo +nightly fmt --all`
- Lint: `cargo clippy --locked --workspace --all-features --all-targets -- -D warnings`
- Check format: `cargo +nightly fmt --all -- --check`

### Local Development Environment
- Start local PostgreSQL: `docker-compose up`
- Full playground environment: `docker compose -f playground/docker-compose.fork.yml up -d`
- For forked network tests, set environment variables: `FORK_MAINNET_URL` and `FORK_GNOSIS_URL`
- Reset playground: `docker-compose -f playground/docker-compose.fork.yml down --remove-orphans --volumes`

## Architecture Overview

### Core Services
- **orderbook** - HTTP API for order management, user interactions, and solver queries. Uses PostgreSQL backend. Multiple instances can run concurrently for horizontal scaling.
- **autopilot** - Protocol driver responsible for auction creation, order inclusion decisions, and settlement competition management. Single instance per deployment.
- **driver** - In-development replacement for `solver` with improved design for co-location with external solvers.
- **solvers** - External solver implementations and solver-related functionality.

### Key Library Crates
- **database** - Shared PostgreSQL database layer and storage logic for orderbook and autopilot
- **shared** - Common functionality between services including utilities, types, and business logic
- **contracts** - Smart contract bindings (migrating from ethcontract to alloy). See `crates/contracts/src/alloy.rs` for alloy bindings, `crates/contracts/build.rs` for legacy ethcontract
- **model** - Serialization models for the orderbook API
- **ethrpc** - Extended Ethereum RPC client with batching layer (`crates/ethrpc/src/alloy/buffering.rs`)
- **chain** - Blockchain interaction utilities
- **number** - Numerical type extensions and conversions for 256-bit integers
- **app-data** - Order metadata validation with 8KB default size limit
- **alerter** - Monitors orderbook metrics for orders that should be solved but aren't
- **testlib** - Shared helpers for writing unit and end-to-end tests
- **observe** - Initialization and helper functions for logging and metrics

### Testing Requirements
- PostgreSQL tests require local database: Run `docker-compose up` first
- Forked network tests require `anvil` (from Foundry) and RPC URLs
- Use `--test-threads 1` for database and E2E tests to avoid conflicts
- CI runs doc-tests, unit tests, DB tests, E2E tests (local and forked), and driver tests

### Workspace Configuration
- Rust Edition 2024
- Uses workspace dependencies for consistency
- Tokio-console support enabled (set `TOKIO_CONSOLE=true` to activate)
- Runtime log filter changes via UNIX socket at `/tmp/log_filter_override_<program_name>_<pid>.sock`

### Development Notes
- Binaries support `--help` for comprehensive command documentation
- OpenAPI documentation available for orderbook, driver, and solver APIs
- Performance profiling available via tokio-console
- Memory allocator: Uses mimalloc for performance

### Playground Environment
- Access full local development stack with CoW Swap UI at http://localhost:8000
- CoW Explorer available at http://localhost:8001
- Orderbook API at http://localhost:8080
- Database admin (Adminer) at http://localhost:8082
- Uses test mnemonic: "test test test test test test test test test test test junk"
- First 10 accounts have 10000 ETH balance by default

## Important Implementation Details

### Contract Bindings Migration
- **New pattern (alloy)**: Use `crates/contracts/src/alloy.rs` with the `bindings!` macro and `deployments!` macro
- **Legacy pattern (ethcontract)**: `crates/contracts/build.rs` - being phased out
- Follow existing style in `alloy.rs` when adding new contract bindings

### Quote System
- **Fast quotes**: Unverified, not stored, immediate expiration (for UI price display)
- **Optimal quotes**: Verified per config, stored in DB, 60s expiration (standard) or 600s (onchain)
- Quote verification modes: `Unverified`, `Prefer`, `EnforceWhenPossible`
- App data size limit: 8KB default (configurable via `--app-data-size-limit`)
- See `quote-competition-analysis.md` and `quote-verification-analysis.md` for details

### RPC Client Architecture
- Batching layer in `crates/ethrpc/src/alloy/buffering.rs` auto-batches individual calls
- Request IDs: Random u32 converted to u64, then incremented per request
- Uses Tower middleware layers: LabelingLayer, InstrumentationLayer, BatchCallLayer