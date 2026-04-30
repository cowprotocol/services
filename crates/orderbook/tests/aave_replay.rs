use {
    alloy::primitives::{U256, address},
    app_data::{AppDataHash, hash_full_app_data},
    model::{
        order::{BuyTokenDestination, OrderData, OrderKind, SellTokenSource},
        signature::Signature,
    },
    simulator::simulation_builder::{
        self,
        Block,
        EthCallInputs,
        ExecutionAmount,
        Prices,
        SettlementSimulator,
        Solver,
    },
    std::{str::FromStr, sync::Arc},
};

/// Replay of a real Aave v3 debt-swap order against a historical mainnet
/// block, exercising the prototype's `SettlementSimulator`-based simulation
/// path end-to-end without involving the orderbook's wall-clock validity
/// check.
///
/// Why this test exists:
///
/// - A full `OrderValidator::validate_and_construct_order` flow uses
///   `SystemTime::now()` to bound `valid_to`, which makes any historical
///   order replay non-deterministic (the test rots as the order expires).
/// - The prototype's value lives in the simulation, not the validity check,
///   so we exercise the simulation directly: build a `SettlementSimulator`
///   against a real RPC, pin the simulation to the block right before
///   settlement, and assert it does not revert.
///
/// Order replayed: an `aave-v3-interface-debt-swap` order
/// `0x7f5df255b55f5eba3034f74acb8e91a04aaf61a755b88c61ad7c61068856f3b2e58acb86761699c1cbc665e6b7e0271503f6336c69f323f8`,
/// sell WETH, buy GHO. The owner is an EIP-1167 minimal proxy that the
/// pre-hook deploys just-in-time.
#[tokio::test]
#[ignore]
async fn aave_debt_swap_replay() {
    let Ok(rpc_url) = std::env::var("MAINNET_RPC_URL") else {
        eprintln!("MAINNET_RPC_URL not set - skipping replay test");
        return;
    };

    let app_data = include_str!("fixtures/aave_replay_app_data.json").trim();
    let inputs = build_replay_simulation(&rpc_url, app_data).await;

    inputs
        .simulate()
        .await
        .expect("simulation must not revert for a healthy production order");
}

/// Same order, but the `flashloan.amount` in `app_data` is rewritten to a
/// value Aave's WETH pool cannot satisfy. The wrapper call to the Aave Pool
/// must revert, and the simulation must propagate that revert.
///
/// This proves the prototype actually executes the flashloan path: if the
/// wrapper call were a silent no-op (e.g. wrong router address), the
/// simulation would not depend on Aave's liquidity at all and would not
/// fail here.
#[tokio::test]
#[ignore]
async fn aave_debt_swap_replay_fails_when_flashloan_oversubscribed() {
    let Ok(rpc_url) = std::env::var("MAINNET_RPC_URL") else {
        eprintln!("MAINNET_RPC_URL not set - skipping replay test");
        return;
    };

    let original = include_str!("fixtures/aave_replay_app_data.json").trim();
    let mut value: serde_json::Value =
        serde_json::from_str(original).expect("fixture must be valid JSON");
    // Way more WETH than Aave can lend. Aave reverts with insufficient
    // liquidity (or similar) before any settlement runs.
    value["metadata"]["flashloan"]["amount"] =
        serde_json::Value::String(U256::MAX.to_string());
    let tampered_app_data = serde_json::to_string(&value).unwrap();

    let inputs = build_replay_simulation(&rpc_url, &tampered_app_data).await;

    let err = inputs
        .simulate()
        .await
        .expect_err("simulation must revert when the flashloan exceeds Aave liquidity");
    tracing::info!(?err, "expected simulation revert observed");
}

/// Builds a simulation pinned to the block right before the Aave debt-swap
/// settlement. The caller controls `full_app_data` so the same wiring
/// supports a positive replay (untouched) and a negative replay (tampered).
async fn build_replay_simulation(rpc_url: &str, full_app_data: &str) -> EthCallInputs {
    // One block before the on-chain settlement transaction. At this block
    // the helper-clone owner contract has no code yet (the pre-hook deploys
    // it), the protocol-adapter factory is live, and Aave v3 has WETH
    // liquidity.
    let fork_block_mainnet = 24_992_051u64;
    let order_owner = address!("e58aCB86761699c1cBC665e6b7E0271503f6336C");
    let sell_token_weth = address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
    let buy_token_gho = address!("40d16fc0246ad3160ccc09b8d0d3a2cd28ae6c2f");
    let sell_amount = U256::from_str("4473358935639875302").unwrap();
    let buy_amount = U256::from_str("10003000000000000000000").unwrap();
    let valid_to = 1_777_542_136u32; // 2026-04-30 09:42:16 UTC
    let signature_hex = include_str!("fixtures/aave_replay_signature.hex").trim();

    let web3 = ethrpc::Web3::new_from_url(rpc_url);
    let provider = web3.provider.clone();
    let chain_id = 1u64;

    let settlement = contracts::GPv2Settlement::Instance::deployed(&provider)
        .await
        .expect("settlement contract not deployed on mainnet?");

    let flash_loan_router = contracts::FlashLoanRouter::deployment_address(&chain_id)
        .expect("FlashLoanRouter deployment address");
    let hooks_trampoline = contracts::HooksTrampoline::deployment_address(&chain_id)
        .expect("HooksTrampoline deployment address");

    let balance_overrider = Arc::new(balance_overrides::BalanceOverrides::new(web3));
    let block_stream = ethrpc::block_stream::mock_single_block(Default::default());

    let simulator = SettlementSimulator::new(
        settlement,
        flash_loan_router,
        hooks_trampoline,
        balance_overrider,
        block_stream,
        None,
    )
    .await
    .expect("failed to create SettlementSimulator");

    let signature_bytes = alloy::primitives::hex::decode(signature_hex.trim_start_matches("0x"))
        .expect("signature fixture must be valid hex");
    let app_data_hash = AppDataHash(hash_full_app_data(full_app_data.as_bytes()));

    let order_data = OrderData {
        sell_token: sell_token_weth,
        buy_token: buy_token_gho,
        receiver: Some(order_owner),
        sell_amount,
        buy_amount,
        valid_to,
        app_data: app_data_hash,
        fee_amount: U256::ZERO,
        kind: OrderKind::Buy,
        partially_fillable: false,
        sell_token_balance: SellTokenSource::Erc20,
        buy_token_balance: BuyTokenDestination::Erc20,
    };

    simulator
        .new_simulation_builder()
        .add_order(
            simulation_builder::Order::new(order_data)
                .with_signature(order_owner, Signature::Eip1271(signature_bytes))
                .with_executed_amount(ExecutionAmount::Full),
        )
        .parameters_from_app_data(full_app_data)
        .expect("parameters_from_app_data should parse the app data")
        .with_prices(Prices::Limit)
        .from_solver(Solver::Fake(None))
        .fund_settlement_contract_with_buy_tokens()
        .at_block(Block::Number(fork_block_mainnet))
        .build()
        .await
        .expect("failed to build simulation")
}
