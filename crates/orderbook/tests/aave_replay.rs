//! Replay of a real Aave v3 debt-swap order against a historical mainnet
//! block, exercising the prototype's `SettlementSimulator`-based simulation
//! path end-to-end without involving the orderbook's wall-clock validity
//! check.
//!
//! Why this test exists:
//!
//! - A full `OrderValidator::validate_and_construct_order` flow uses
//!   `SystemTime::now()` to bound `valid_to`, which makes any historical
//!   order replay non-deterministic (the test rots as the order expires).
//! - The prototype's value lives in the simulation, not the validity check,
//!   so we exercise the simulation directly: build a `SettlementSimulator`
//!   against a real RPC, pin the simulation to the block right before
//!   settlement, and assert it does not revert.
//!
//! Order replayed: an `aave-v3-interface-debt-swap` order
//! `0x7f5df255...69f323f8`, owner `0xe58aCB86...3f6336C` (an EIP-1167 minimal
//! proxy that the pre-hook deploys just-in-time), sell WETH, buy GHO,
//! settled at mainnet block 24992052.
//!
//! Run with `cargo nextest run --test aave_replay -p orderbook
//! --run-ignored ignored-only` and `MAINNET_RPC_URL` set to an archive node.
//! Without the env var, the test silently skips so CI without an archive
//! endpoint stays green.

use {
    alloy::primitives::{Address, U256, address},
    app_data::{AppDataHash, hash_full_app_data},
    model::{
        order::{BuyTokenDestination, OrderData, OrderKind, SellTokenSource},
        signature::Signature,
    },
    simulator::simulation_builder::{
        self,
        Block,
        ExecutionAmount,
        Prices,
        SettlementSimulator,
        Solver,
    },
    std::{str::FromStr, sync::Arc},
};

/// One block before the on-chain settlement transaction. At this block the
/// helper-clone owner contract has no code yet (the pre-hook deploys it),
/// the protocol-adapter factory is live, and Aave v3 has WETH liquidity.
const FORK_BLOCK_MAINNET: u64 = 24992051;

const ORDER_OWNER: Address = address!("e58aCB86761699c1cBC665e6b7E0271503f6336C");
const SELL_TOKEN_WETH: Address = address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
const BUY_TOKEN_GHO: Address = address!("40d16fc0246ad3160ccc09b8d0d3a2cd28ae6c2f");
const SELL_AMOUNT: u128 = 4_473_358_935_639_875_302;
const VALID_TO: u32 = 1_777_542_136;
const BUY_AMOUNT_DECIMAL: &str = "10003000000000000000000";

const FULL_APP_DATA: &str = include_str!("fixtures/aave_replay_app_data.json");
const SIGNATURE_HEX: &str = include_str!("fixtures/aave_replay_signature.hex");

#[tokio::test]
#[ignore]
async fn aave_debt_swap_replay() {
    let Ok(rpc_url) = std::env::var("MAINNET_RPC_URL") else {
        eprintln!("MAINNET_RPC_URL not set - skipping replay test");
        return;
    };

    let web3 = ethrpc::Web3::new_from_url(&rpc_url);
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

    let signature_bytes = alloy::primitives::hex::decode(
        SIGNATURE_HEX.trim().trim_start_matches("0x"),
    )
    .expect("signature fixture must be valid hex");

    let app_data_json = FULL_APP_DATA.trim();
    let app_data_hash = AppDataHash(hash_full_app_data(app_data_json.as_bytes()));

    let order_data = OrderData {
        sell_token: SELL_TOKEN_WETH,
        buy_token: BUY_TOKEN_GHO,
        receiver: Some(ORDER_OWNER),
        sell_amount: U256::from(SELL_AMOUNT),
        buy_amount: U256::from_str(BUY_AMOUNT_DECIMAL).unwrap(),
        valid_to: VALID_TO,
        app_data: app_data_hash,
        fee_amount: U256::ZERO,
        kind: OrderKind::Buy,
        partially_fillable: false,
        sell_token_balance: SellTokenSource::Erc20,
        buy_token_balance: BuyTokenDestination::Erc20,
    };

    let inputs = simulator
        .new_simulation_builder()
        .add_order(
            simulation_builder::Order::new(order_data)
                .with_signature(ORDER_OWNER, Signature::Eip1271(signature_bytes))
                .with_executed_amount(ExecutionAmount::Full),
        )
        .parameters_from_app_data(app_data_json)
        .expect("parameters_from_app_data should parse the fixture")
        .with_prices(Prices::Limit)
        .from_solver(Solver::Fake(None))
        .fund_settlement_contract_with_buy_tokens()
        .at_block(Block::Number(FORK_BLOCK_MAINNET))
        .build()
        .await
        .expect("failed to build simulation");

    match inputs.simulate().await {
        Ok(returndata) => {
            tracing::info!(
                "simulation succeeded, returndata: 0x{}",
                alloy::primitives::hex::encode(&returndata)
            );
        }
        Err(err) => {
            panic!(
                "simulation must not revert for a healthy production order. Error: {err:?}"
            );
        }
    }
}
