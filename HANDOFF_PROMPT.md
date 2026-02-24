# Pod Network Shadow Mode Integration — Full Context Handoff

You are working on the `cowprotocol/services` Rust monorepo (CoW Protocol backend). The repo is at `/Users/aryan/CoW/services`. You are currently on branch `feat/pod-network-3`.

## Background: What Is Pod Network Integration?

CoW Protocol has an **autopilot** (centralized arbitrator) that collects solutions from multiple **drivers** (each running a solver engine), picks winners using combinatorial auction logic, and tells winners to settle on chain.

The **pod network** is a decentralized coordination layer. The goal: instead of the autopilot being the single source of truth, each driver submits its bids to pod, then after the deadline fetches ALL bids from pod, runs the SAME winner selection algorithm locally, and determines if it won.

**Phase 1 (this PR) = Shadow Mode**: The existing autopilot flow is unchanged. But additionally, each driver submits bids to pod, fetches bids, runs local winner selection, and **LOGs** the outcome. We compare the logs to verify the driver picks the same winner as the autopilot. No functional change to live protocol.

## What The PR Does (Current State of `feat/pod-network-3`)

The branch has 9 commits on top of a merge with `main` at commit `1b17a61ad` (from ~3 months ago). The pod team wrote the code, Martin (senior engineer) reviewed it, comments were addressed.

### Architecture

```
Normal flow (unchanged):
  Autopilot → /solve → Driver → SolverEngine → solutions → Driver picks best → responds to Autopilot

Shadow pod flow (new, runs in background after solve):
  1. Driver serializes its best solution as a SolveResponse
  2. Driver calls pod_sdk::AuctionClient::submit_bid(auction_id, deadline, score, serialized_data)
  3. Driver calls wait_for_auction_end(deadline) — blocks until auction closes
  4. Driver calls fetch_bids(auction_id) — gets all drivers' serialized solutions
  5. Driver deserializes each bid as a SolveResponse
  6. Driver runs SolverArbitrator::arbitrate() — same algorithm as autopilot
  7. Driver LOGs "[pod] local winner selected" with auction_id, address, score
  8. Autopilot LOGs "[pod] CoW winner selected" with same fields
  9. A bash script compares the two log lines to verify they match
```

**Critical for consensus**: Both autopilot and driver sort solutions by `keccak256(deterministic_serialization)` BEFORE running winner selection. This ensures identical input ordering → identical output.

### Files Changed (Complete List)

**New dependency**: `pod-sdk = "0.5.1"` added to workspace `Cargo.toml`

#### winner-selection crate (shared library)
- `crates/winner-selection/src/lib.rs` — Added `pub mod solution_hash;`
- `crates/winner-selection/src/solution_hash.rs` — **NEW**: Defines `HashableSolution` and `HashableTradedOrder` traits + `hash_solution()` function. Both autopilot and driver implement these traits on their respective Solution types so they produce identical hashes.
- `crates/winner-selection/src/arbitrator.rs` — Added `#[derive(Clone)]` to `Arbitrator`

#### autopilot crate
- `crates/autopilot/src/domain/auction/order.rs` — Added `Ord, PartialOrd` derives and `AsRef<[u8]>` impl to `OrderUid`
- `crates/autopilot/src/domain/competition/mod.rs` — Added `HashableSolution` impl for `Solution`, `HashableTradedOrder` impl for `TradedOrder`. Made `Score.0` field `pub`.
- `crates/autopilot/src/domain/competition/winner_selection.rs` — Added `bids.sort_by_cached_key(hash_solution)` at start of `arbitrate()`. Added `[pod] CoW arbitration completed` and `[pod] CoW winner selected` log lines.
- `crates/autopilot/src/domain/eth/mod.rs` — Added `AsRef<[u8]>` for `TokenAddress`, `From<WrappedNativeToken> for Address`
- `crates/autopilot/src/domain/fee/mod.rs` — Changed `use` to `pub use` (exposes fee types)

#### driver crate (main integration)
- `crates/driver/Cargo.toml` — Added deps: `pod-sdk`, `autopilot`, `winner-selection`, `primitive-types`, `derivative`
- `crates/driver/src/domain/competition/mod.rs` — Added 4 async methods to `Competition`: `pod_flow()`, `pod_solution_submission()`, `pod_fetch_bids()`, `local_winner_selection()`. Added pod flow call in `solve()` method after best settlement is cached. Made `Solved` and `Amounts` `Clone`.
- `crates/driver/src/domain/competition/solver_winner_selection.rs` — **NEW** (256 lines): `SolverArbitrator` wrapping `winsel::Arbitrator`, `Bid<State>` type-state, conversion impls (`Auction → AuctionContext`, `FeePolicy`, `dto::Solution → winsel::Solution`)
- `crates/driver/src/domain/competition/auction.rs` — Added `iter_keys_values()` to `Tokens`
- `crates/driver/src/infra/api/mod.rs` — Changed `solver: Solver` to `solver: Arc<Solver>` in API State
- `crates/driver/src/infra/api/routes/solve/dto/mod.rs` — Made `solve_response` module `pub(crate)`
- `crates/driver/src/infra/api/routes/solve/dto/solve_response.rs` — Added `Deserialize` to structs, made fields `pub`, added `HashableSolution`/`HashableTradedOrder` impls
- `crates/driver/src/infra/solver/mod.rs` — Added `pod_provider: Option<PodProvider>` and `arbitrator: SolverArbitrator` fields to `Solver`. Added `build_pod_provider()`, `make_signer()`, `pod()`, `arbitrator()` methods. Changed `Solver` derive from `#[derive(Debug)]` to `#[derive(Derivative)] #[derivative(Debug)]`.
- `crates/driver/src/infra/config/file/mod.rs` — Added `pod: Option<pod::config::Config>` to TOML config struct
- `crates/driver/src/infra/config/file/load.rs` — Plumbed `pod_config` through to solver Config
- `crates/driver/src/infra/config/mod.rs` — Added `pod` field to `infra::Config`
- `crates/driver/src/infra/mod.rs` — Added `pub mod pod;`
- `crates/driver/src/infra/pod/mod.rs` — **NEW**: `pub mod config;`
- `crates/driver/src/infra/pod/config.rs` — **NEW**: Config struct with `endpoint: Url` and `auction_contract_address: pod_sdk::alloy_primitives::Address`

#### e2e / playground / scripts
- `crates/e2e/src/setup/colocation.rs` — Added `[pod]` config section to generated driver TOML
- `crates/e2e/src/setup/config/mod.rs` — **NEW**: `pub mod pod;`
- `crates/e2e/src/setup/config/pod.rs` — **NEW**: Pod endpoint + auction contract constants
- `crates/e2e/src/setup/mod.rs` — Added `mod config;`
- `playground/driver.toml` — Added `[pod]` config section
- `playground/driver2.toml` — **NEW**: Second driver config with different private key
- `playground/baseline2.toml` — **NEW**: Second baseline solver config
- `playground/docker-compose.fork.yml` — Added driver2 + baseline2 services, updated autopilot/orderbook to reference both
- `playground/docker-compose.non-interactive.yml` — Same changes for non-interactive mode
- `scripts/e2e_cow_pod_match_winners.sh` — **NEW**: Bash script to grep logs and verify pod winner matches CoW winner

## Known Bugs To Fix

### 1. Hardcoded WETH Address (severity: medium)
In `crates/driver/src/infra/solver/mod.rs`, `Solver::try_new()`:
```rust
let arbitrator = SolverArbitrator::new(
    10,
    WrappedNativeToken::from(address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")),
); // WETH
```
This only works on Ethereum mainnet. Should use `eth.contracts().weth_address()` which is already available in the method.

### 2. `score.unwrap()` Panic (severity: high)
In `crates/driver/src/domain/competition/mod.rs`, `pod_solution_submission()`:
```rust
let pod_auction_value = score.unwrap().score.0;
```
If `score` is `None`, this panics. Should handle the `None` case gracefully (log + return Ok).

### 3. Hardcoded `max_winners = 10`
Should ideally come from config or match autopilot's value.

## What Has Changed on `main` Since This Branch Diverged

The branch diverged at commit `1b17a61ad` (~3 months ago). There are **115 commits** on main since then. Here are the ones that will likely cause merge conflicts or require adaptation:

### HIGH IMPACT — Will Almost Certainly Cause Conflicts

1. **`#4174` — Remove direct dependency on derivative crate**
   - The pod PR ADDS `derivative` as a dep for `driver/Cargo.toml` and uses `#[derive(Derivative)]` on `Solver`
   - Main has REMOVED `derivative` from the workspace
   - **Fix**: Replace `Derivative` usage with manual `Debug` impl or use a different approach

2. **`#4106` — Remove ethcontract+web3+primitive-types**
   - The pod PR ADDS `primitive-types` as a dep for `driver/Cargo.toml`
   - Main has REMOVED `primitive-types` from the workspace
   - **Fix**: Use alloy primitives instead

3. **`#4142` — Extract common utils, serialization to shared and serde-ext**
   - The driver's `solve_response.rs` uses `crate::util::serialize` for `U256` serialization
   - This may have been moved to `shared` or `serde-ext`
   - **Fix**: Update import paths

4. **`#4164` — Upgrade axum 0.6 → 0.8**
   - The pod PR modifies `driver/src/infra/api/mod.rs` (changing `solver` to `Arc<Solver>`)
   - The axum upgrade likely restructured this same file
   - **Fix**: Reapply the `Arc<Solver>` change on top of the axum 0.8 code

5. **`#4172` — Upgrade reqwest to 0.13**
   - May affect `Solver`'s reqwest client construction

6. **`#4158` — Move fee policy configuration from CLI args to config file**
   - The pod PR's `solver_winner_selection.rs` has `From<&FeePolicy> for winsel::primitives::FeePolicy`
   - Fee policy types may have moved
   - **Fix**: Update import paths and conversion impls

7. **`#4147` — Add TOML configuration to the autopilot**
   - Autopilot config structure changed significantly
   - The pod PR modifies autopilot's `winner_selection.rs` and `fee/mod.rs`
   - **Fix**: Verify changes still apply cleanly

### MEDIUM IMPACT — May Require Minor Adjustments

8. **`#4168` — Bump alloy to 1.7.3** — May affect `pod-sdk` compatibility
9. **`#4175` — Bump sqlx to 0.8 and bigdecimal to 0.4** — Cargo.toml conflicts
10. **`#4160` — Only stream 1 `/solve` request body in driver** — Driver API changes
11. **`#4159` — send `/solve` requests with ref-counted body** — Autopilot changes
12. **`#4049` — Add haircut configuration** — New driver config fields

## Step-by-Step Plan

### Phase 1: Rebase and Get It Compiling

1. **Create a working branch**:
   ```bash
   git checkout main
   git pull origin main
   git checkout -b feat/pod-network-rebase
   git cherry-pick <each pod commit> # or rebase feat/pod-network-3 onto main
   ```
   Alternatively, do an interactive rebase:
   ```bash
   git checkout feat/pod-network-3
   git rebase -i main
   ```

2. **Resolve conflicts during rebase** — expect conflicts in:
   - `Cargo.toml` / `crates/driver/Cargo.toml` (dependency changes)
   - `crates/driver/src/infra/api/mod.rs` (axum upgrade)
   - `crates/driver/src/infra/solver/mod.rs` (multiple refactors)
   - `crates/driver/src/infra/config/file/load.rs` (config restructuring)
   - `crates/autopilot/src/domain/competition/winner_selection.rs`
   - `crates/autopilot/src/domain/fee/mod.rs`

3. **Fix dependency issues**:
   - Remove `primitive-types` dep from `driver/Cargo.toml` — use alloy types instead
   - Remove `derivative` dep from `driver/Cargo.toml` — replace `#[derive(Derivative)] #[derivative(Debug)]` with a manual `impl Debug` for `Solver` that skips `pod_provider` and `arbitrator` fields
   - Update `serialize::U256` import paths if they moved to `shared` or `serde-ext`

4. **Fix the known bugs** listed above (hardcoded WETH, unwrap panic)

5. **Verify compilation**:
   ```bash
   cargo check -p driver
   cargo check -p autopilot
   cargo check -p winner-selection
   cargo check --workspace
   ```

### Phase 2: Verify Tests Pass

1. **Run existing tests**:
   ```bash
   cargo test -p winner-selection
   cargo test -p autopilot -- winner_selection
   cargo test -p driver
   ```

2. **Run full workspace tests**:
   ```bash
   cargo test --workspace
   ```

3. **Check clippy**:
   ```bash
   cargo clippy --workspace -- -D warnings
   ```

### Phase 3: Test the Pod Flow

1. **Playground test** (if you have Docker):
   ```bash
   cd playground
   ENV=fork docker compose -f docker-compose.fork.yml up --build
   ```
   Then check logs for `[pod]` prefixed lines:
   - `[pod] pod provider built with wallet` — pod SDK initialized
   - `[pod] bid submission succeeded` — bid sent to pod
   - `[pod] fetched bids` — bids received from pod
   - `[pod] local arbitration completed` — local winner selection ran
   - `[pod] local winner selected` — local winner logged

2. **Run the verification script**:
   ```bash
   docker compose -f playground/docker-compose.fork.yml logs 2>&1 | bash scripts/e2e_cow_pod_match_winners.sh
   ```

## Key Files to Study

If you need to understand the codebase better, these are the most important files:

| File | What it does |
|------|-------------|
| `crates/driver/src/domain/competition/mod.rs` | The main `Competition::solve()` method + pod flow methods |
| `crates/driver/src/domain/competition/solver_winner_selection.rs` | Driver-side arbitrator, Bid type-state, conversion impls |
| `crates/driver/src/infra/solver/mod.rs` | Solver struct with pod provider and arbitrator |
| `crates/driver/src/infra/api/routes/solve/dto/solve_response.rs` | SolveResponse DTO with Serialize + Deserialize |
| `crates/autopilot/src/domain/competition/winner_selection.rs` | Autopilot arbitrator (the "ground truth" for comparison) |
| `crates/winner-selection/src/solution_hash.rs` | Deterministic solution hashing for consensus |
| `crates/winner-selection/src/arbitrator.rs` | Core arbitration algorithm (shared library) |
| `crates/driver/src/infra/pod/config.rs` | Pod configuration struct |

## Important Crate Architecture

```
winner-selection (shared library)
├── arbitrator.rs — Core winner selection algorithm
├── solution_hash.rs — Deterministic hashing for consensus ordering
├── solution.rs — Solution type with state machine
├── auction.rs — AuctionContext
└── primitives.rs — FeePolicy, Quote, etc.

autopilot (centralized coordinator)
├── domain/competition/winner_selection.rs — Wraps winsel::Arbitrator + Bid type-state
├── domain/competition/mod.rs — Solution, TradedOrder + HashableSolution impls
└── domain/eth/ — TokenAddress, WrappedNativeToken

driver (per-solver process)
├── domain/competition/mod.rs — Competition::solve() + pod_flow()
├── domain/competition/solver_winner_selection.rs — SolverArbitrator + Bid type-state (mirrors autopilot's)
├── infra/solver/mod.rs — Solver struct with PodProvider
├── infra/pod/config.rs — Pod config
└── infra/api/routes/solve/dto/solve_response.rs — SolveResponse DTO
```

## Success Criteria

1. `cargo check --workspace` passes
2. `cargo test --workspace` passes
3. `cargo clippy --workspace` passes
4. The pod flow compiles and doesn't panic (the bugs are fixed)
5. Log lines are properly emitted with `[pod]` prefix
6. The `e2e_cow_pod_match_winners.sh` script structure makes sense with the log format

## What NOT to Change

- Do NOT modify the core winner selection algorithm in `winner-selection` crate
- Do NOT change the existing autopilot flow (non-pod code paths)
- Do NOT change how `Competition::solve()` works for the normal (non-pod) path
- Do NOT remove the `[pod]` log prefixes — they're used by the verification script
- The pod flow should remain fire-and-forget (tokio::spawn) — it must not block the normal solve flow
