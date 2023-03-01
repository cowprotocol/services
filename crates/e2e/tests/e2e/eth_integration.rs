use {
    crate::setup::*,
    ethcontract::prelude::{Address, U256},
    model::{
        order::{OrderBuilder, OrderKind, BUY_ETH_ADDRESS},
        quote::{OrderQuoteRequest, OrderQuoteSide, SellAmount},
        signature::EcdsaSigningScheme,
    },
    secp256k1::SecretKey,
    shared::ethrpc::Web3,
    web3::signing::SecretKeyRef,
};

#[tokio::test]
#[ignore]
async fn local_node_eth_integration() {
    run_test(eth_integration).await;
}

async fn eth_integration(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(to_wei(1)).await;
    let [trader_a, trader_b] = onchain.make_accounts(to_wei(1)).await;

    // Create & mint tokens to trade, pools for fee connections
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(100_000), to_wei(100_000))
        .await;
    token.mint(trader_a.address(), to_wei(51)).await;
    token.mint(trader_b.address(), to_wei(51)).await;

    // Approve GPv2 for trading
    tx!(
        trader_a.account(),
        token.approve(onchain.contracts().allowance, to_wei(51))
    );
    tx!(
        trader_b.account(),
        token.approve(onchain.contracts().allowance, to_wei(51))
    );

    let trader_a_eth_balance_before = web3.eth().balance(trader_a.address(), None).await.unwrap();
    let trader_b_eth_balance_before = web3.eth().balance(trader_b.address(), None).await.unwrap();

    let services = Services::new(onchain.contracts()).await;
    services.start_autopilot(vec![]);
    services.start_api(vec![]).await;

    let quote = |sell_token, buy_token| {
        let services = &services;
        async move {
            let request = OrderQuoteRequest {
                sell_token,
                buy_token,
                from: Address::default(),
                side: OrderQuoteSide::Sell {
                    sell_amount: SellAmount::AfterFee { value: to_wei(43) },
                },
                ..Default::default()
            };
            services.submit_quote(&request).await
        }
    };
    quote(token.address(), BUY_ETH_ADDRESS).await.unwrap();
    // Eth is only supported as the buy token
    let (status, body) = quote(BUY_ETH_ADDRESS, token.address()).await.unwrap_err();
    assert_eq!(status, 400, "{body}");

    // Place Orders
    assert_ne!(onchain.contracts().weth.address(), BUY_ETH_ADDRESS);
    let order_buy_eth_a = OrderBuilder::default()
        .with_kind(OrderKind::Buy)
        .with_sell_token(token.address())
        .with_sell_amount(to_wei(50))
        .with_fee_amount(to_wei(1))
        .with_buy_token(BUY_ETH_ADDRESS)
        .with_buy_amount(to_wei(49))
        .with_valid_to(model::time::now_in_epoch_seconds() + 300)
        .sign_with(
            EcdsaSigningScheme::Eip712,
            &onchain.contracts().domain_separator,
            SecretKeyRef::from(&SecretKey::from_slice(trader_a.private_key()).unwrap()),
        )
        .build()
        .into_order_creation();
    services.create_order(&order_buy_eth_a).await.unwrap();
    let order_buy_eth_b = OrderBuilder::default()
        .with_kind(OrderKind::Sell)
        .with_sell_token(token.address())
        .with_sell_amount(to_wei(50))
        .with_fee_amount(to_wei(1))
        .with_buy_token(BUY_ETH_ADDRESS)
        .with_buy_amount(to_wei(49))
        .with_valid_to(model::time::now_in_epoch_seconds() + 300)
        .sign_with(
            EcdsaSigningScheme::Eip712,
            &onchain.contracts().domain_separator,
            SecretKeyRef::from(&SecretKey::from_slice(trader_b.private_key()).unwrap()),
        )
        .build()
        .into_order_creation();
    services.create_order(&order_buy_eth_b).await.unwrap();

    tracing::info!("Waiting for trade.");
    wait_for_condition(TIMEOUT, || async { services.solvable_orders().await == 2 })
        .await
        .unwrap();

    services.start_old_driver(solver.private_key(), vec![]);

    let trade_happened = || async {
        let balance_a = web3.eth().balance(trader_a.address(), None).await.unwrap();
        let balance_b = web3.eth().balance(trader_b.address(), None).await.unwrap();
        balance_a != trader_a_eth_balance_before && balance_b != trader_b_eth_balance_before
    };
    wait_for_condition(TIMEOUT, trade_happened).await.unwrap();

    // Check matching
    let trader_a_eth_balance_after = web3.eth().balance(trader_a.address(), None).await.unwrap();
    let trader_b_eth_balance_after = web3.eth().balance(trader_b.address(), None).await.unwrap();
    assert_eq!(
        trader_a_eth_balance_after - trader_a_eth_balance_before,
        to_wei(49)
    );
    assert_eq!(
        trader_b_eth_balance_after - trader_b_eth_balance_before,
        49_800_747_827_208_136_744_u128.into()
    );
}
