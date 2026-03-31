# Plan: Proper Tx Gas Simulation for Baseline Solver Hooks

## Context

The baseline solver (`crates/solvers`) estimates gas using static, per-liquidity-source costs plus a fixed `solution_gas_offset`. Order hooks (pre/post interactions set by users) are transmitted in the auction DTO but **completely ignored in gas estimation**.

For **sell=buy orders** (sell token == buy token — no routing needed, 100% of execution gas comes from hooks), the gas is currently `eth::Gas(U256::ZERO) + solution_gas_offset`. The hooks may consume hundreds of thousands of additional gas units that are never charged as fee. When baseline solver is the only one solving such orders, CoW Protocol loses money.

For **regular orders with hooks**, the routing gas is estimated but hook gas is still 0.

The fix: add hook gas into the solver's fee computation by using the same settlement simulation approach already established in `crates/orderbook/src/order_simulator.rs`.

---

## Root Cause

1. `crates/solvers-dto/src/auction.rs` (`Order`) **does** have `pre_interactions` and `post_interactions` fields — the data arrives from the driver
2. `crates/solvers/src/api/routes/solve/dto/auction.rs` does **not** map them when building the domain `Order`
3. `crates/solvers/src/domain/order.rs`'s `Order` has **no** hook fields at all
4. Gas formula in `baseline.rs:208` (`sell=buy`) and `:229` (regular) does not add any hook gas

---

## Implementation Plan

### Step 1 — Thread hooks into the solver domain

**File: `crates/solvers/src/domain/order.rs`**

Add to the `Order` struct:
```rust
pub pre_interactions: Vec<eth::Interaction>,
pub post_interactions: Vec<eth::Interaction>,
```

`eth::Interaction` is already defined in `crates/solvers/src/domain/eth/mod.rs`.

**File: `crates/solvers/src/api/routes/solve/dto/auction.rs`**

In the `order::Order { ... }` construction block (around line 43), map:
```rust
pre_interactions: order.pre_interactions.iter().map(|i| eth::Interaction {
    target: i.target,
    value: eth::Ether(i.value),
    calldata: i.call_data.clone(),
}).collect(),
post_interactions: order.post_interactions.iter().map(|i| eth::Interaction {
    target: i.target,
    value: eth::Ether(i.value),
    calldata: i.call_data.clone(),
}).collect(),
```

---

### Step 2 — Add dependencies to `solvers`

**File: `crates/solvers/Cargo.toml`**

Add:
```toml
simulator = { workspace = true }
balance-overrides = { workspace = true }
```

(The `simulator` crate already exists as a workspace member.)

---

### Step 3 — Create `TxGasEstimator` in the solvers infra

**New file: `crates/solvers/src/infra/tx_gas.rs`**

Important clarification on simulation methods in `SwapSimulator`:
- `simulate_settle_call()` uses `eth_call` → returns `Bytes` (settlement return data), **no gas info**
- `simulate_swap_with_solver()` uses `eth_call` via `Solver` contract → returns swap output amounts, **no gas info**
- For actual gas estimation, `eth_estimateGas` must be used (this is how `simulator::Simulator.gas()` works in the driver)

Estimates the full gas usage of a proposed solution (including order hooks, swap interactions, and settlement overhead) by simulation. Follows the same post-processing pattern as `orderbook/src/order_simulator.rs`:

1. Holds a `SwapSimulator` and a raw `ethrpc::Web3` provider
2. Exposes `async fn estimate(solution: &solution::Single) -> eth::Gas` (or equivalent)
3. Inside:
   a. Calls `fake_swap(&query)` to build an `EncodedSwap`
   b. **Injects order hooks** into the encoded swap's interactions (following the `add_interactions()` pattern from `orderbook/src/order_simulator.rs:162-178`):
      ```rust
      // Prepend order pre-interactions (before any existing fake_swap pre-interactions)
      let order_pre = order.pre_interactions.iter().map(encode_interaction);
      encoded_swap.settlement.interactions.pre = order_pre
          .chain(std::mem::take(&mut encoded_swap.settlement.interactions.pre))
          .collect();
      // Append order post-interactions
      encoded_swap.settlement.interactions.post.extend(
          order.post_interactions.iter().map(encode_interaction)
      );
      ```
   c. Applies state overrides to `encoded_swap.overrides` (following `OrderSimulator.add_state_overrides()`):
      - Authenticator → `AnyoneAuthenticator::DEPLOYED_BYTECODE`
      - Solver (fake random address) → large ETH balance
      - `query.from` (order owner) → `Trader::DEPLOYED_BYTECODE`
      - Settlement contract → out_token balance override via `self.balance_overrides.state_override()`
   d. Builds a `TransactionRequest` (same as `simulate_settle_call()` does internally)
   e. Calls `eth_estimateGas` with state overrides:
      ```rust
      web3.provider
          .estimate_gas(tx_request)
          .overrides(state_overrides)
          .await
      ```
4. On failure/revert, returns `eth::Gas(U256::ZERO)` — falls back to existing behaviour

The estimator is constructed from:
- `web3: ethrpc::Web3` (from a node URL)
- `settlement: contracts::alloy::GPv2Settlement::Instance` (from `chain_id`)
- `native_token: Address` (already in `Config.weth`)
- `balance_overrides: Arc<dyn BalanceOverriding>`
- `current_block: ethrpc::block_stream::CurrentBlockWatcher`
- `gas_limit: u64` (large constant e.g. `15_000_000`)

---

### Step 4 — Update baseline config

**File: `crates/solvers/src/infra/config/baseline.rs`**

Add optional gas-simulation config:
```toml
[gas-simulation]
node-url = "https://..."
```

When both `chain-id` and `node-url` are present (for gas simulation), build a `TxGasEstimator`. Pass it through `solver::Config`:
```rust
pub tx_gas_estimator: Option<TxGasEstimator>,
```

The WETH address (already in config) becomes `native_token`. Settlement address comes from `contracts::Contracts::for_chain(chain_id).settlement`.

---

### Step 5 — Wire gas estimation into the baseline solver

**File: `crates/solvers/src/domain/solver/baseline.rs`**

Add to `Inner`:
```rust
tx_gas_estimator: Option<Arc<TxGasEstimator>>,
```

In `compute_solution` (the `async |request: Request|` closure):

The `TxGasEstimator` simulates the **entire** settlement tx (routing + hooks + settlement overhead), so its result replaces the static estimate entirely — `solution_gas_offset` is NOT added on top (it's already measured by the simulation). When simulation is unavailable or fails, fall back to existing static estimate.

1. **sell=buy branch** (line ~208):
```rust
let gas = if let Some(ref est) = self.tx_gas_estimator {
    // full simulation: includes hooks + settlement overhead
    est.estimate(&order, input.amount, output.amount).await
        .filter(|g| !g.0.is_zero())
        .unwrap_or_else(|| eth::Gas(U256::ZERO) + self.solution_gas_offset)
} else {
    eth::Gas(U256::ZERO) + self.solution_gas_offset
};
```

2. **routing branch** (line ~229):
```rust
let gas = if let Some(ref est) = self.tx_gas_estimator {
    est.estimate(&order, input.amount, output.amount).await
        .filter(|g| !g.0.is_zero())
        .unwrap_or_else(|| route.gas() + self.solution_gas_offset)
} else {
    route.gas() + self.solution_gas_offset
};
```

The fee calculation at line ~252 already uses `solution.gas`, so no further changes are needed there — it will automatically use the corrected gas.

---

## Critical Files

| File | Change |
|------|--------|
| `crates/solvers/src/domain/order.rs` | Add `pre_interactions`, `post_interactions` to `Order` |
| `crates/solvers/src/api/routes/solve/dto/auction.rs` | Map DTO hooks to domain |
| `crates/solvers/Cargo.toml` | Add `simulator`, `balance-overrides` deps |
| `crates/solvers/src/infra/config/baseline.rs` | Add optional gas-simulation node URL |
| `crates/solvers/src/infra/tx_gas.rs` | **New** — `TxGasEstimator` wrapping `SwapSimulator` |
| `crates/solvers/src/domain/solver/baseline.rs` | Add `tx_gas_estimator`, use simulated gas in gas formula |
| `crates/solvers/src/infra/mod.rs` | Export new `tx_gas` module |

---

## Key Existing Code to Reuse

- `crates/orderbook/src/order_simulator.rs:162-178` — `add_interactions()`: the canonical pattern for injecting order hooks into an `EncodedSwap` after `fake_swap()` — copy this exactly
- `crates/orderbook/src/order_simulator.rs:105-158` — `add_state_overrides()`: authenticator + ETH + trader + token balance overrides — model the `TxGasEstimator` directly after this
- `crates/simulator/src/swap_simulator.rs` — `fake_swap()`, `SwapSimulator::new()`
- `crates/simulator/src/encoding.rs` — `EncodedInteraction`, `InteractionEncoding` trait
- `crates/solvers/src/infra/dex/simulator.rs` — Existing pattern for Alloy-based RPC calls in the solvers crate

---

## Verification

1. **Unit test**: Add a test in `crates/solvers/src/tests/cases/` (e.g., `tx_gas.rs`) that exercises the `sell=buy` path with mock interactions and verifies `solution.gas` > `solution_gas_offset`.

2. **Integration**: Run existing solver tests:
   ```
   cargo nextest run -p solvers
   ```

3. **E2E**: Run E2E test for a sell=buy order with hooks to confirm fee is non-zero and covers actual gas:
   ```
   cargo nextest run -p e2e local_node --run-ignored ignored-only --test-threads 1
   ```

4. **Manual verification**: In a local playground with a sell=buy limit order that has a pre-interaction hook, confirm the baseline solver's solution includes a non-zero gas fee that matches the actual hook gas from the driver simulation.

---

## Implementation Log

**Branch:** `tx-gas-simulation` — implemented 2026-03-31

### Files changed

| File | Change |
|------|--------|
| `crates/solvers/src/domain/eth/mod.rs` | Added `Clone` derive to `Interaction` |
| `crates/solvers/src/domain/order.rs` | Added `pre_interactions` and `post_interactions: Vec<eth::Interaction>` to `Order` |
| `crates/solvers/src/api/routes/solve/dto/auction.rs` | Maps `pre_interactions`/`post_interactions` from DTO `InteractionData` to domain on order construction |
| `crates/solvers/Cargo.toml` | Added `simulator` and `balance-overrides` workspace deps |
| `crates/solvers/src/infra/mod.rs` | Exported `pub mod tx_gas` |
| `crates/solvers/src/infra/tx_gas.rs` | **New** — `TxGasEstimator`: wraps `SwapSimulator`, builds fake settlement via `fake_swap()`, injects order hooks (mirrors `add_interactions()` from `orderbook/src/order_simulator.rs`), applies state overrides (mirrors `add_state_overrides()`), calls `simulate_settle_call()` then `eth_estimateGas` |
| `crates/solvers/src/infra/config/baseline.rs` | Added optional `gas_simulation_node_url: Option<Url>` TOML field under `[gas-simulation]`; builds `TxGasEstimator` via `BalanceOverrides::new(web3)` + `SwapSimulator::new(...)` when URL is present; threads it into `solver::Config` |
| `crates/solvers/src/domain/solver/baseline.rs` | Added `tx_gas_estimator: Option<Arc<TxGasEstimator>>` to `Config` and `Inner`; sell=buy and routing branches both use simulation when available, fallback to static estimates on `None` or zero result |

### Deviations from plan

- **`BuyTokenDestination`**: Plan referenced a non-existent `TokenHolder` variant; actual enum only has `Erc20` (default) and `Internal`. Used `Erc20`.
- **`estimate_gas` return type**: Alloy's `Provider::estimate_gas` returns `EthCall<N, U64, u64>` (resolves to `u64`), not `U256`. Fixed type annotation and converted with `U256::from(gas)`.
- **`Provider` trait import**: `estimate_gas` is a trait method on `Provider`; needed explicit `use alloy::providers::Provider` in `tx_gas.rs`.
- **`eth::Interaction` not `Clone`**: Required adding `#[derive(Clone)]` to `Interaction` in `domain/eth/mod.rs` because `Order` derives `Clone`.
- **No modification to `SwapSimulator`**: The plan discussed adding `estimate_settle_gas` to `SwapSimulator`, but it was avoided by reusing the `tx`/`overrides` returned from the existing `simulate_settle_call()` and passing them to `estimate_gas` on the provider directly.
