//! Forked-mainnet replay of a real Aave v3 debt-swap order.
//!
//! This test takes a production Aave debt-swap order that settled on mainnet
//! at block 24992052 and resubmits it through our orderbook running on a
//! mainnet fork at block 24992051 (one before settlement). The orderbook is
//! configured with `Eip1271SimulationMode::Enforce`, so the prototype's
//! validation-time simulation must accept the order for the API to return
//! HTTP 201.
//!
//! What this exercises end-to-end:
//!
//! - `WrapperConfig::Flashloan` routing through the deployed `FlashLoanRouter`
//!   on the fork, against the real Aave v3 Pool as the lender.
//! - The user-signed pre-hook deploying the EIP-1167 helper clone via the
//!   protocol-adapter factory and funding it with the loaned WETH.
//! - The signature_validator's pre-interaction simulation deploying the same
//!   clone in time for `isValidSignature` to be called on real bytecode.
//! - The settlement transferring sell tokens from the now-funded helper.
//! - The post-hook running the loan repayment path.
//!
//! Order details (from cow API + DB):
//!   uid:    0x7f5df255b55f5eba3034f74acb8e91a04aaf61a755b88c61ad7c61068856f3b2
//!           e58acb86761699c1cbc665e6b7e0271503f6336c69f323f8
//!   owner:  0xe58aCB86761699c1cBC665e6b7E0271503f6336C
//!   sell:   4.473358935639875302 WETH
//!   buy:    10003 GHO
//!   class:  limit, kind: buy, partiallyFillable: false
//!   signed: eip1271
//!   appCode: aave-v3-interface-debt-swap
//!   settled at block 24992052, status: fulfilled.

use {
    alloy::{
        hex,
        primitives::{Address, U256, address},
    },
    configs::{orderbook::Eip1271SimulationMode, test_util::TestDefault},
    e2e::setup::{OnchainComponents, Services, run_forked_test_with_block_number},
    model::{
        order::{OrderCreation, OrderCreationAppData, OrderKind},
        signature::Signature,
    },
    number::units::EthUnit,
    shared::web3::Web3,
    std::str::FromStr,
};

/// One block before the settlement transaction (24992052) of the replayed
/// order.
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
async fn forked_node_mainnet_eip1271_aave_replay() {
    run_forked_test_with_block_number(
        forked_aave_replay,
        std::env::var("FORK_URL_MAINNET").expect("FORK_URL_MAINNET must be set to run forked tests"),
        FORK_BLOCK_MAINNET,
    )
    .await;
}

async fn forked_aave_replay(web3: Web3) {
    let mut onchain = OnchainComponents::deployed(web3.clone()).await;
    let [solver] = onchain.make_solvers_forked(1u64.eth()).await;

    let mut orderbook_config = configs::orderbook::Configuration::test_default();
    orderbook_config
        .order_simulation
        .as_mut()
        .expect("test_default enables order_simulation")
        .eip1271_simulation_mode = Eip1271SimulationMode::Enforce;

    let services = Services::new(&onchain).await;
    services
        .start_protocol_with_args(
            configs::autopilot::Configuration::test("test_solver", solver.address()),
            orderbook_config,
            solver,
        )
        .await;

    let signature_bytes = hex::decode(SIGNATURE_HEX.trim().trim_start_matches("0x"))
        .expect("signature fixture must be valid hex");

    let order = OrderCreation {
        sell_token: SELL_TOKEN_WETH,
        buy_token: BUY_TOKEN_GHO,
        receiver: Some(ORDER_OWNER),
        sell_amount: U256::from(SELL_AMOUNT),
        buy_amount: U256::from_str(BUY_AMOUNT_DECIMAL).unwrap(),
        valid_to: VALID_TO,
        fee_amount: U256::ZERO,
        kind: OrderKind::Buy,
        partially_fillable: false,
        from: Some(ORDER_OWNER),
        signature: Signature::Eip1271(signature_bytes),
        app_data: OrderCreationAppData::Full {
            full: FULL_APP_DATA.trim().to_string(),
        },
        ..Default::default()
    };

    let uid = services
        .create_order(&order)
        .await
        .expect("orderbook should accept the replayed Aave order");
    tracing::info!(?uid, "order accepted");

    let stored = services.get_order(&uid).await.unwrap();
    assert_eq!(stored.metadata.uid, uid);
    assert_eq!(stored.metadata.owner, ORDER_OWNER);
}
