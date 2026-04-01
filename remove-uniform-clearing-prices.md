# Plan: Remove Uniform Clearing Prices (UCP) from Backend Services

## Context

Uniform Clearing Prices (UCP) are no longer part of the core CoW Protocol mechanism since CIP-67. They add artificial restrictions on batched solutions, complicate onboarding new solvers, and create subtle dependencies in accounting and UI. The team has decided to remove them entirely from the autopilot and driver.

UCP currently serve one internal purpose: computing an "executed fee" (the difference in surplus before/after fee application). This fee is used in the explorer (web-squad will switch to showing protocol/partner fees directly) and solver accounting (solver-squad will adapt buffer accounting).

This plan covers the backend-squad scope: removing UCP from the codebase across all layers.

## Key Findings

1. **Winner selection already works without UCP** — `arbitrator.rs:216-223` assigns `_uniform_sell_price`/`_uniform_buy_price` but never uses them. All scoring uses `calculate_custom_prices_from_executed()` from `executed_sell`/`executed_buy`.

2. **Autopilot already receives executed amounts** — `solve_response.rs:50-51` includes per-trade `executed_sell`/`executed_buy` in `TradedOrder`. The `clearing_prices` field is forwarded to winner selection but effectively dead.

3. **UCP in calldata are dead weight** — Trade indices point to custom prices (`encoding.rs:168-169`), not UCP entries (`encoding.rs:60-68`). The settlement contract never references UCP.

---

## PR Structure and Dependency Graph

```
PR 1: winner-selection cleanup (trivial, zero risk)
  │
  v
PR 2: autopilot-driver API cleanup (remove clearing_prices from solve response)
  │
  v
PR 3: autopilot settlement analysis + calldata encoding  ◄── COORDINATION GATE
  │    (hard coupling: encoder and decoder must change together)
  │    Requires: web-squad ready for executed_fee semantic change
  │    Requires: solver-squad ready for buffer accounting change
  │
  v
PR 4: driver internal cleanup (remove UCP from domain model)
  │
  v
PR 5: solver API change (backward compatible)  ◄── COORDINATION GATE
  │    Requires: external solver operators notified, transition period
  │
  v
PR 6: internal solver update + legacy removal
```

---

## PR 1: Remove UCP from Winner Selection

**Risk:** Zero — variables are literally prefixed with `_`.
**Deployable independently:** Yes.

### Files
- `crates/winner-selection/src/solution.rs` — Remove `prices: HashMap<Address, U256>` field, `prices()` accessor, update `new()` and `with_state()`
- `crates/winner-selection/src/arbitrator.rs:216-223` — Remove dead `_uniform_sell_price`/`_uniform_buy_price` lookups in `compute_order_score()`
- `crates/autopilot/src/domain/competition/winner_selection.rs:148-164` — Stop passing prices when constructing `winsel::Solution`
- `crates/autopilot/src/run_loop.rs:495-500` — Stop populating `clearing_prices` in run loop solution construction

### Review Checkpoint
- [ ] `cargo nextest run -p winner-selection`
- [ ] `cargo nextest run -p autopilot`
- [ ] `cargo clippy --locked -p winner-selection -p autopilot --all-features --all-targets -- -D warnings`

---

## PR 2: Remove `clearing_prices` from Autopilot-Driver API

**Depends on:** PR 1 (winner selection no longer needs prices).
**Deployable independently:** Yes — driver and autopilot deploy from same repo.

### Files
- `crates/driver/src/infra/api/routes/solve/dto/solve_response.rs:56-61,80` — Remove `clearing_prices` field from `Solution` DTO and `Solution::new()`
- `crates/driver/src/domain/competition/mod.rs:1013-1018` — Remove `prices` field from `Solved` struct
- `crates/driver/src/domain/competition/mod.rs:518` — Stop populating `prices` in `Solved` construction
- `crates/driver/src/domain/competition/solution/settlement.rs:341-350` — Remove `Settlement::prices()` method
- `crates/autopilot/src/infra/solvers/dto/solve.rs:200-205,275` — Remove `clearing_prices` from autopilot's solve DTO and `into_domain()`
- `crates/autopilot/src/domain/competition/mod.rs` — Remove `prices` from autopilot's `Solution` type, drop `prices()` method

### Review Checkpoint
- [ ] `cargo nextest run -p driver -p autopilot`
- [ ] `cargo clippy --locked -p driver -p autopilot --all-features --all-targets -- -D warnings`
- [ ] Verify solve response JSON no longer includes `clearingPrices` field

---

## COORDINATION GATE: Web-squad and Solver-squad

Before PR 3 can merge:

- **Web-squad** must be ready to handle the `executed_fee` semantic change on Explorer. Instead of showing total fee (network + protocol), they will show only protocol/partner fees in the exact token they were charged in.
- **Solver-squad** must be ready to adapt buffer accounting to treat network fee imbalances the same as slippage imbalances.

---

## PR 3: Autopilot Settlement Analysis + Calldata Encoding

This PR combines the autopilot-side analysis changes with the calldata encoding change. These are **hard-coupled**: the calldata encoder (driver) and decoder (autopilot) must agree on format. Since they deploy from the same repo, they belong in the same PR.

**Depends on:** PR 2 + coordination gate above.
**Deployable independently:** Yes (single deployment of driver + autopilot).

### Part A: Autopilot Settlement Analysis (decoder side)

#### Step 1: Remove `Prices` struct from `transaction/mod.rs`

- **Delete** the `Prices` struct (lines 252-256)
- **Change** `EncodedTrade.prices` type from `Prices` to `ClearingPrices`
- **Remove** uniform index lookup (lines 130-135: `uniform_sell_token_index`, `uniform_buy_token_index`)
- **Simplify** price construction (lines 162-171) to just:
  ```rust
  prices: ClearingPrices {
      sell: clearing_prices[sell_token_index],
      buy: clearing_prices[buy_token_index],
  },
  ```
- **Update** `ClearingPrices` doc comment — remove "Uniform" from "Uniform clearing prices at which the trade was executed"

#### Step 2: Update `trade/math.rs` — core math changes

- **Update import**: Remove `Prices` from `settlement::transaction::{ClearingPrices, Prices}`
- **Change** `Trade.prices` field type from `Prices` to `ClearingPrices`
- **All `self.prices.custom.*` references become `self.prices.*`** (in `sell_amount()`, `buy_amount()`, `surplus_over_limit_price()`, `surplus_over_quote()`, `protocol_fees()`)
- **Delete** `surplus_over_limit_price_before_fee()` (lines 300-308)
- **Delete** `fee()` (lines 134-141) — no longer computable without uniform prices, and no longer needed
- **Rewrite `fee_in_ether()`**: New signature adds `fee_policies` parameter. Body: call `protocol_fees(fee_policies)`, sum the fee amounts, convert total to ETH via native price of surplus token
- **Rewrite `fee_in_sell_token()`**: New signature adds `fee_policies` parameter. Body: call `protocol_fees(fee_policies)`, sum fees, convert to sell token via `fee_into_sell_token()`
- **Fix `fee_into_sell_token()`**: Change `self.prices.uniform.buy/sell` to `self.prices.buy/sell`
- **Add TODO comment** near fee methods:
  ```rust
  // TODO: This fee accounting should eventually be removed and offloaded to the
  // solvers team entirely.
  ```

#### Step 3: Update `trade/mod.rs` — trade types and delegation

- **Update imports**: Remove `Prices`, import `transaction::ClearingPrices` if needed
- **Change** `Fulfillment.prices` type from `Prices` to `transaction::ClearingPrices`
- **Change** `Jit.prices` type from `super::transaction::Prices` to `super::transaction::ClearingPrices`
- **Simplify JIT construction** (lines 117-127): Remove the `if surplus_capturing { ... } else { ... }` around prices — just use `trade.prices` directly (both branches are now equivalent)
- **Update `fee_in_ether()` signature** to `pub fn fee_in_ether(&self, auction: &super::Auction) -> ...` and delegate as `math::Trade::from(self).fee_in_ether(&auction.prices, &auction.orders)`
- **Update `fee_breakdown()`**: Change `trade.fee_in_sell_token()` to `trade.fee_in_sell_token(&auction.orders)`
- **Update `FeeBreakdown.total` doc comment**: Change "network fee + protocol fee" to "sum of protocol fees"

#### Step 4: Update `settlement/mod.rs` — callers and tests

- **Update `Settlement::fee_in_ether()`** (line 98): Change `trade.fee_in_ether(&self.auction.prices)` to `trade.fee_in_ether(&self.auction)`
- **Update tests**:
  - `settlement` test (line 862): Update `fee_in_ether` call signature. Expected value changes from `6752697350740628` to `0` (order has empty fee policies, so sum of protocol fees = 0)
  - `settlement_with_liquidity_order_and_user_order` test (line 1351): Update call signature (expected value already 0)
  - `ws_executed_amounts` helper (line 441): Change `trade.prices.custom.sell/buy` to `trade.prices.sell/buy`
  - Any other tests referencing `fee_in_ether` or constructing `Prices` structs

### Part B: Calldata Encoding (encoder side)

- `crates/driver/src/domain/competition/solution/encoding.rs`
  - Remove lines 60-68 (uniform clearing price vector encoding)
  - Adjust capacity hint on line 50
  - Trade custom price encoding (lines 163-169) stays unchanged
  - Trade indices naturally adjust since there are fewer preceding entries

### DB/API Impact
- `executed_fee` in `order_execution` table: was "network + protocol fees", becomes "protocol fees only"
- Orders with no fee policies will have `executed_fee = 0`
- `is_sell_order_filled()` is UNAFFECTED (uses `sum_fee` from `trades` table)

### E2E Tests to Update
- `crates/e2e/tests/e2e/ethflow.rs:196` — `executed_fee > 0` check
- `crates/e2e/tests/e2e/protocol_fee.rs:337` — `!executed_fee.is_zero()` check
- `crates/e2e/tests/e2e/partially_fillable_pool.rs:133` — `!executed_fee.is_zero()` check
- Fix: check `executed_buy_amount > 0` instead, or only assert nonzero for orders with protocol fee policies

### Backward Compatibility
- Old settled transactions have UCP in calldata but are already processed in DB
- The decoder uses `trade.sellTokenIndex`/`buyTokenIndex` for custom prices — this works regardless of whether UCP are present in the array
- No reprocessing issues: old calldata still decodes correctly (custom price indices are valid)

### Review Checkpoint
- [ ] `cargo nextest run -p autopilot`
- [ ] `cargo nextest run -p driver`
- [ ] `cargo clippy --locked -p autopilot -p driver --all-features --all-targets -- -D warnings`
- [ ] E2E tests pass: `cargo nextest run -p e2e local_node --test-threads 1 --run-ignored ignored-only`
- [ ] Verify that decoding old calldata (with UCP) still works correctly
- [ ] Confirm web-squad and solver-squad are ready before merge

---

## PR 4: Remove UCP from Driver Domain Model

This is the most architecturally significant PR. The driver currently uses UCP to:
- Compute `sell_amount()`/`buy_amount()` for the non-target side of each trade
- Derive custom clearing prices from uniform ones
- Apply protocol fees
- Score solutions
- Merge solutions (congruent prices check)

Without UCP, all these computations must derive from executed amounts directly.

**Depends on:** PR 3 (calldata no longer encodes/decodes UCP).
**Deployable independently:** Yes.

### Files
- `crates/driver/src/domain/competition/solution/mod.rs`
  - Remove `prices: Prices` field from `Solution`
  - Remove `clearing_prices()` and `clearing_price()` methods
  - Update `scoring()` (lines 268-304) to derive custom prices from executed amounts (like winner-selection already does) instead of from UCP
  - Update solution merging logic (price scaling/congruence check)
- `crates/driver/src/domain/competition/solution/trade.rs`
  - Remove `ClearingPrices` struct (uniform)
  - Rename `CustomClearingPrices` to `ClearingPrices`
  - Refactor `sell_amount()`, `buy_amount()`, `custom_prices()` to work without UCP parameter
  - The trade must carry both executed sides, not just `executed` (target side)
- `crates/driver/src/domain/competition/solution/scoring.rs`
  - Already works with `CustomClearingPrices`, minimal changes
  - Update `Trade::new()` parameters if `CustomClearingPrices` is renamed
- `crates/driver/src/domain/competition/solution/encoding.rs:77-91`
  - Currently looks up uniform prices per trade to pass to `custom_prices()`
  - Instead derive custom prices from executed amounts directly
- `crates/driver/src/domain/competition/solution/settlement.rs:307-311`
  - `Settlement::orders()` computes `executed_sell`/`executed_buy` from UCP — derive from trade's own data instead
- `crates/driver/src/domain/competition/solution/fee.rs`
  - `with_protocol_fees()` uses UCP to compute `ClearingPrices` — refactor to work from executed amounts
- `crates/driver/src/domain/quote.rs`
  - Quote computation uses UCP for haircut-adjusted prices — refactor
- `crates/driver/src/infra/solver/dto/solution.rs`
  - Stop reading `prices` from solver response (still accept for backward compat, just ignore)

### Design Note
The driver currently receives `executed_amount` (single value, target side) + UCP from solver, and derives both sides. Without UCP, the solver DTO still provides `prices` (for now), but the driver should compute both sides from prices internally and store them on the trade, then stop depending on UCP for anything else. This is a stepping stone to PR 5 where solvers provide both sides directly.

### Review Checkpoint
- [ ] `cargo nextest run -p driver`
- [ ] `RUST_MIN_STACK=3145728 cargo nextest run -p driver --test-threads 1 --run-ignored ignored-only`
- [ ] `cargo clippy --locked -p driver --all-features --all-targets -- -D warnings`
- [ ] Driver scoring matches winner-selection scoring for test cases

---

## COORDINATION GATE: External Solver Operators

Before PR 5 can be deployed with the legacy path removed:

- **Announce** the new solver API format with documentation
- **Publish timeline** (e.g., 4-8 weeks transition period)
- **All external solver operators** must migrate to providing per-trade executed amounts

---

## PR 5: Replace Solver API `prices` with Per-Trade Executed Amounts

Change the solver API contract with backward compatibility.

**Depends on:** PR 4.
**Deployable independently:** Yes (backward compatible).

### Files
- `crates/solvers-dto/src/solution.rs`
  - Make `prices` optional with `#[serde(default)]`: `pub prices: HashMap<Address, U256>`
  - Add to `Fulfillment`: `pub executed_sell: Option<U256>`, `pub executed_buy: Option<U256>`
  - Add to `JitTrade`: `pub executed_sell: Option<U256>`, `pub executed_buy: Option<U256>`
- `crates/driver/src/infra/solver/dto/solution.rs`
  - Handle both formats: if per-trade amounts present, use them; otherwise fall back to UCP + `executed_amount` computation (legacy path)

### Backward Compatibility
```
// New format (preferred):
{ "trades": [{ "kind": "fulfillment", "order": "0x...",
  "executedSell": "100", "executedBuy": "200" }] }

// Old format (legacy, still accepted):
{ "prices": {"0xA": "2", "0xB": "1"},
  "trades": [{ "kind": "fulfillment", "order": "0x...", "executedAmount": "100" }] }
```

### Review Checkpoint
- [ ] `cargo nextest run -p driver`
- [ ] Old-format solver responses still produce correct settlements
- [ ] New-format solver responses produce identical settlements
- [ ] Solver API documentation updated

---

## PR 6: Internal Solver Update + Legacy Removal

Update the internal solver to produce the new format. After all external solvers have migrated, remove the legacy path.

**Depends on:** PR 5 + all external solvers migrated.
**Deployable independently:** Yes.

### Files
- `crates/solvers/src/domain/solution.rs` — Replace `ClearingPrices` with per-trade executed amounts
- `crates/solver/src/settlement/settlement_encoder.rs` — Remove `TokenReference::Indexed` path
- `crates/solvers-dto/src/solution.rs` — Remove `prices` field entirely (after migration deadline)
- `crates/driver/src/infra/solver/dto/solution.rs` — Remove legacy UCP->amounts computation path

### Review Checkpoint
- [ ] `cargo nextest run -p solvers -p solver -p driver`
- [ ] Full E2E test suite passes
- [ ] `cargo clippy --locked --workspace --all-features --all-targets -- -D warnings`
- [ ] `cargo +nightly fmt --all`

---

## Verification (After All PRs)

1. Full workspace compilation: `cargo check --workspace`
2. Full test suite: `cargo nextest run`
3. E2E local tests: `cargo nextest run -p e2e local_node --test-threads 1 --run-ignored ignored-only`
4. E2E forked tests: `cargo nextest run -p e2e forked_node --test-threads 1 --run-ignored ignored-only`
5. Verify `executed_fee` DB values: only protocol fees, 0 for orders without fee policies
6. Verify calldata gas savings on test settlements (no UCP overhead)
