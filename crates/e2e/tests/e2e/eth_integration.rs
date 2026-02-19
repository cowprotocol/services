use {
    ::alloy::{primitives::Address, providers::Provider},
    e2e::setup::*,
    ethrpc::alloy::CallBuilderExt,
    model::{
        order::{BUY_ETH_ADDRESS, OrderCreation, OrderKind},
        quote::{OrderQuoteRequest, OrderQuoteSide, SellAmount},
        signature::EcdsaSigningScheme,
    },
    number::{nonzero::NonZeroU256, units::EthUnit},
    shared::web3::Web3,
};

#[tokio::test]
#[ignore]
async fn local_node_eth_integration() {
    run_test(eth_integration).await;
}

async fn eth_integration(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(1u64.eth()).await;
    let [trader_a, trader_b] = onchain.make_accounts(1u64.eth()).await;

    // Create & mint tokens to trade, pools for fee connections
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(100_000u64.eth(), 100_000u64.eth())
        .await;
    token.mint(trader_a.address(), 51u64.eth()).await;
    token.mint(trader_b.address(), 51u64.eth()).await;

    // Approve GPv2 for trading

    token
        .approve(onchain.contracts().allowance, 51u64.eth())
        .from(trader_a.address())
        .send_and_watch()
        .await
        .unwrap();

    token
        .approve(onchain.contracts().allowance, 51u64.eth())
        .from(trader_b.address())
        .send_and_watch()
        .await
        .unwrap();

    let trader_a_eth_balance_before = web3.provider.get_balance(trader_a.address()).await.unwrap();

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    let quote = |sell_token, buy_token| {
        let services = &services;
        async move {
            let request = OrderQuoteRequest {
                sell_token,
                buy_token,
                from: Address::default(),
                side: OrderQuoteSide::Sell {
                    sell_amount: SellAmount::AfterFee {
                        value: NonZeroU256::try_from(43u64.eth()).unwrap(),
                    },
                },
                ..Default::default()
            };
            services.submit_quote(&request).await
        }
    };
    quote(*token.address(), BUY_ETH_ADDRESS).await.unwrap();
    // Eth is only supported as the buy token
    let (status, body) = quote(BUY_ETH_ADDRESS, *token.address()).await.unwrap_err();
    assert_eq!(status, 400, "{body}");

    // Place Orders
    assert_ne!(*onchain.contracts().weth.address(), BUY_ETH_ADDRESS);
    let order_buy_eth_a = OrderCreation {
        kind: OrderKind::Buy,
        sell_token: *token.address(),
        sell_amount: 50u64.eth(),
        buy_token: BUY_ETH_ADDRESS,
        buy_amount: 49u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader_a.signer,
    );
    services.create_order(&order_buy_eth_a).await.unwrap();
    let order_buy_eth_b = OrderCreation {
        kind: OrderKind::Sell,
        sell_token: *token.address(),
        sell_amount: 50u64.eth(),
        buy_token: BUY_ETH_ADDRESS,
        buy_amount: 49u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader_b.signer,
    );
    services.create_order(&order_buy_eth_b).await.unwrap();

    tracing::info!("Waiting for trade.");
    onchain.mint_block().await;
    let trade_happened = || async {
        let balance_a = web3.provider.get_balance(trader_a.address()).await.unwrap();
        let balance_b = web3.provider.get_balance(trader_b.address()).await.unwrap();

        let trader_a_eth_decreased = (balance_a - trader_a_eth_balance_before) == 49u64.eth();
        let trader_b_eth_increased = balance_b >= 49u64.eth();
        trader_a_eth_decreased && trader_b_eth_increased
    };
    wait_for_condition(TIMEOUT, trade_happened).await.unwrap();
}
