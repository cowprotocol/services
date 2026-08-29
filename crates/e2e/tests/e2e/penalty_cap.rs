use {
    configs::{
        autopilot::{Configuration, penalty_cap::PenaltyCapConfig, solver::Solver},
        test_util::TestDefault,
    },
    e2e::setup::*,
    ethrpc::alloy::CallBuilderExt,
    model::{
        order::{OrderCreation, OrderKind},
        signature::EcdsaSigningScheme,
    },
    number::units::EthUnit,
    shared::web3::Web3,
};

#[tokio::test]
#[ignore]
async fn local_node_penalty_cap() {
    run_test(penalty_cap).await;
}

/// Places an order with the penalty cap feature enabled and asserts
/// that the order gets a penalty cap assigned in the auction.
async fn penalty_cap(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;
    let [solver] = onchain.make_solvers(1u64.eth()).await;
    let [trader] = onchain.make_accounts(1u64.eth()).await;
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    token_a.mint(trader.address(), 100u64.eth()).await;
    token_a
        .approve(onchain.contracts().allowance, 100u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    let services = Services::new(&onchain).await;
    services
        .start_protocol_with_args(
            Configuration {
                drivers: vec![Solver::test("test_solver", solver.address())],
                penalty_cap: Some(PenaltyCapConfig {
                    default_factor: 0.0004.try_into().unwrap(),
                    absolute_cap_usd: 20.,
                    // Use WETH as the USD reference token: its native price
                    // is 1 by definition, so the bound is really 20 ETH here.
                    // The test only cares about the plumbing.
                    usd_reference_token: *onchain.contracts().weth.address(),
                    overrides: vec![],
                }),
                ..Configuration::test_no_drivers()
            },
            configs::orderbook::Configuration::test_default(),
            solver,
        )
        .await;

    let order = OrderCreation {
        sell_token: *token_a.address(),
        sell_amount: 5u64.eth(),
        buy_token: *token_b.address(),
        buy_amount: 1u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    let uid = services.create_order(&order).await.unwrap();

    // The order shows up in the auction with a non-zero penalty cap assigned.
    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;
        let auction = services.get_auction().await.auction;
        auction.orders.iter().any(|order| {
            order.uid == uid && order.penalty_cap_native.is_some_and(|cap| !cap.is_zero())
        })
    })
    .await
    .unwrap();

    // Once the auction's competition is saved, the order's penalty cap gets
    // persisted alongside it for penalty accounting.
    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;
        let caps = crate::database::penalty_caps_of_order(services.db(), &uid).await;
        caps.iter()
            .any(|cap| *cap > bigdecimal::BigDecimal::from(0))
    })
    .await
    .unwrap();
}
