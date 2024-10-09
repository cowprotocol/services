use {
    e2e::{setup::*, tx},
    ethcontract::prelude::{Address, U256},
    model::{
        order::{OrderCreation, OrderKind, BUY_ETH_ADDRESS},
        quote::{OrderQuoteRequest, OrderQuoteSide, SellAmount},
        signature::EcdsaSigningScheme,
    },
    number::nonzero::U256 as NonZeroU256,
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

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    // We force the block to start before the test, so the auction is not cut by the
    // block in the middle of the operations, creating uncertainty
    onchain.mint_block().await;

    let quote = |sell_token, buy_token| {
        let services = &services;
        async move {
            let request = OrderQuoteRequest {
                sell_token,
                buy_token,
                from: Address::default(),
                side: OrderQuoteSide::Sell {
                    sell_amount: SellAmount::AfterFee {
                        value: NonZeroU256::try_from(to_wei(43)).unwrap(),
                    },
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
    let order_buy_eth_a = OrderCreation {
        kind: OrderKind::Buy,
        sell_token: token.address(),
        sell_amount: to_wei(50),
        buy_token: BUY_ETH_ADDRESS,
        buy_amount: to_wei(49),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader_a.private_key()).unwrap()),
    );
    services.create_order(&order_buy_eth_a).await.unwrap();
    let order_buy_eth_b = OrderCreation {
        kind: OrderKind::Sell,
        sell_token: token.address(),
        sell_amount: to_wei(50),
        buy_token: BUY_ETH_ADDRESS,
        buy_amount: to_wei(49),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader_b.private_key()).unwrap()),
    );
    services.create_order(&order_buy_eth_b).await.unwrap();

    tracing::info!("Waiting for trade.");
    onchain.mint_block().await;
    let trade_happened = || async {
        let balance_a = web3.eth().balance(trader_a.address(), None).await.unwrap();
        let balance_b = web3.eth().balance(trader_b.address(), None).await.unwrap();

        let trader_a_eth_decreased = (balance_a - trader_a_eth_balance_before) == to_wei(49);
        let trader_b_eth_increased = balance_b >= to_wei(49);
        trader_a_eth_decreased && trader_b_eth_increased
    };
    wait_for_condition(TIMEOUT, trade_happened).await.unwrap();
}
