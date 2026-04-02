# Plan: E2E Test for tx_gas_simulation Gas Comparison

## Context

The `tx-gas-simulation` branch adds a `TxGasEstimator` to the baseline solver that simulates the full settlement transaction (including order hooks) via `eth_estimateGas` to get an accurate gas estimate. Without it, the solver uses a static estimate (`route.gas() + solution_gas_offset`) that ignores hook costs.

Goal: add an E2E test that compares `gas_amount` from the quote response with and without `tx_gas_estimator` enabled, asserting the simulated value is higher.

## Key Findings

- `gas_amount` in `OrderQuoteResponse.quote` is the gas units estimate that comes from the baseline solver's solution — this is what changes with gas simulation.
- The baseline solver config supports `gas-simulation-node-url`, but currently also requires `chain-id` to look up the canonical settlement contract address. Local anvil deploys contracts at non-canonical addresses, so we must add a `gas-simulation-settlement` override.
- The driver listens on port `0.0.0.0:11088` by default. When the driver tokio task is aborted, the `TcpListener` is dropped and (with `SO_REUSEADDR`) the port is immediately reusable.
- `services.start_api(config)` preserves `order_quoting` from the passed config but overrides `shared` with contract addresses. We can use this directly without calling `start_protocol`.
- `colocation::start_driver(...)` returns a `JoinHandle` that lets us abort the driver mid-test.

## Files to Modify

### 1. `crates/solvers/src/infra/config/baseline.rs`

Add field to `Config` struct:
```rust
/// Explicit settlement contract address for gas simulation. When provided,
/// `chain-id` is not required for gas simulation. Useful for local test
/// environments where contracts are deployed at non-canonical addresses.
gas_simulation_settlement: Option<eth::Address>,
```

Change tx_gas_estimator creation (lines 83–108) to use explicit address when available:
```rust
let tx_gas_estimator = if let Some(url) = config.gas_simulation_node_url {
    let settlement_addr = if let Some(addr) = config.gas_simulation_settlement {
        addr
    } else {
        let chain_id = config.chain_id.expect(
            "invalid configuration: `chain-id` is required when `gas-simulation.node-url` \
             is set and `gas-simulation.settlement` is not provided",
        );
        contracts::Contracts::for_chain(chain_id).settlement
    };
    // rest of block unchanged, replace `contracts::Contracts::for_chain(chain_id).settlement`
    // with `settlement_addr`
```

### 2. `crates/e2e/src/setup/colocation.rs`

Add new public function after `start_baseline_solver_with_haircut`:
```rust
pub async fn start_baseline_solver_with_gas_simulation(
    name: String,
    account: TestAccount,
    weth: Address,
    base_tokens: Vec<Address>,
    max_hops: usize,
    merge_solutions: bool,
    settlement: Address,
) -> SolverEngine {
    let encoded_base_tokens = encode_base_tokens(base_tokens.clone());
    let config_file = config_tmp_file(format!(
        r#"
weth = "{weth:?}"
base-tokens = [{encoded_base_tokens}]
max-hops = {max_hops}
max-partial-attempts = 5
native-token-price-estimation-amount = "100000000000000000"
uni-v3-node-url = "http://localhost:8545"
gas-simulation-node-url = "http://localhost:8545"
gas-simulation-settlement = "{settlement:?}"
        "#,
    ));
    let endpoint = start_solver(config_file, "baseline".to_string()).await;
    SolverEngine {
        name, endpoint, account, base_tokens, merge_solutions,
        haircut_bps: 0, submission_keys: vec![], forwarder_contract: None,
    }
}
```

### 3. `crates/e2e/tests/e2e/hooks.rs`

Add test registration + function. New imports needed:
- `e2e::setup::colocation`
- `configs::order_quoting::{ExternalSolver, OrderQuoting}`
- `configs::test_util::TestDefault`

Test structure:
```
local_node_tx_gas_simulation → run_test(tx_gas_simulation)

async fn tx_gas_simulation(web3: Web3) {
  1. OnchainComponents::deploy(web3.clone()), make_solvers, make_accounts
  2. deploy_tokens_with_weth_uni_v2_pools (100k/100k ETH pools)
  3. token.mint + token.approve for trader
  4. Deploy Counter contract; build pre_hook and post_hook using
     counter.increment() with estimate_gas()
  5. Build quote_request with hooks in app_data JSON

  // Phase 1: without gas simulation
  6. colocation::start_baseline_solver(...) → solver_no_sim
  7. colocation::start_driver(contracts, vec![solver_no_sim], UniswapV2, false) → driver_no_sim JoinHandle
  8. services.start_api(orderbook config with ExternalSolver at 11088).await
  9. services.submit_quote(&quote_request).await → gas_no_sim = quote.gas_amount

  // Phase 2: with gas simulation
  10. driver_no_sim.abort(); driver_no_sim.await.ok()
  11. colocation::start_baseline_solver_with_gas_simulation(
          ..., *onchain.contracts().gp_settlement.address()
      ) → solver_with_sim
  12. colocation::start_driver(contracts, vec![solver_with_sim], UniswapV2, false)
  13. wait_for_condition(TIMEOUT, || async {
          reqwest::get("http://localhost:11088/healthz").await.is_ok()
      }).await
  14. services.submit_quote(&quote_request).await → gas_with_sim = quote.gas_amount

  15. assert!(gas_with_sim > gas_no_sim, ...)
}
```

## Verification

Run the new test:
```bash
cargo nextest run -p e2e tx_gas_simulation --test-threads 1 --run-ignored ignored-only --failure-output final
```

Then format:
```bash
cargo +nightly fmt --all
```

The test should pass with `gas_with_sim > gas_no_sim`. The Counter.increment() hook calls each cost ~20-30k gas extra, which simulation captures but the static estimator misses.
