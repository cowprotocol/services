use {
    ::alloy::primitives::U256,
    e2e::setup::*,
    ethrpc::alloy::{CallBuilderExt, EvmProviderExt},
    model::{
        order::{OrderCreation, OrderKind},
        quote::{OrderQuoteRequest, OrderQuoteSide, SellAmount},
        signature::EcdsaSigningScheme,
    },
    number::units::EthUnit,
    shared::{domain::eth::NonZeroU256, web3::Web3},
    std::ops::DerefMut,
};

#[tokio::test]
#[ignore]
async fn local_node_place_order_with_quote_basic() {
    run_test(place_order_with_quote).await;
}

#[tokio::test]
#[ignore]
async fn local_node_disabled_same_sell_and_buy_token_order_feature() {
    run_test(disabled_same_sell_and_buy_token_order_feature).await;
}

async fn place_order_with_quote(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(10u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    onchain
        .contracts()
        .weth
        .approve(onchain.contracts().allowance, 3u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .deposit()
        .from(trader.address())
        .value(3u64.eth())
        .send_and_watch()
        .await
        .unwrap();

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    services.start_protocol(solver.clone()).await;

    // Disable auto-mine so we don't accidentally mine a settlement
    web3.provider
        .evm_set_automine(false)
        .await
        .expect("Must be able to disable automine");

    tracing::info!("Quoting");
    let quote_sell_amount = 1u64.eth();
    let quote_request = OrderQuoteRequest {
        from: trader.address(),
        sell_token: *onchain.contracts().weth.address(),
        buy_token: *token.address(),
        side: OrderQuoteSide::Sell {
            sell_amount: SellAmount::BeforeFee {
                value: NonZeroU256::try_from(quote_sell_amount).unwrap(),
            },
        },
        ..Default::default()
    };
    let quote_response = services.submit_quote(&quote_request).await.unwrap();
    tracing::debug!(?quote_response);
    assert!(quote_response.id.is_some());
    assert!(quote_response.verified);

    let quote_metadata =
        crate::database::quote_metadata(services.db(), quote_response.id.unwrap()).await;
    assert!(quote_metadata.is_some());
    tracing::debug!(?quote_metadata);

    tracing::info!("Placing order");
    let balance = token.balanceOf(trader.address()).call().await.unwrap();
    assert_eq!(balance, U256::ZERO);
    let order = OrderCreation {
        quote_id: quote_response.id,
        sell_token: *onchain.contracts().weth.address(),
        sell_amount: quote_sell_amount,
        buy_token: *token.address(),
        buy_amount: quote_response.quote.buy_amount,
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    let order_uid = services.create_order(&order).await.unwrap();

    tracing::info!("Order quote verification");
    let order_quote = database::orders::read_quote(
        services.db().acquire().await.unwrap().deref_mut(),
        &database::byte_array::ByteArray(order_uid.0),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(quote_response.verified, order_quote.verified);
    assert_eq!(quote_metadata.unwrap().0, order_quote.metadata);
}

async fn disabled_same_sell_and_buy_token_order_feature(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(10u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    token.mint(trader.address(), 10u64.eth()).await;

    token
        .approve(onchain.contracts().allowance, 10u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    services.start_protocol(solver.clone()).await;

    // Disable auto-mine so we don't accidentally mine a settlement
    web3.provider
        .evm_set_automine(false)
        .await
        .expect("Must be able to disable automine");

    tracing::info!("Quoting");
    let quote_sell_amount = 1u64.eth();
    let quote_request = OrderQuoteRequest {
        from: trader.address(),
        sell_token: *token.address(),
        buy_token: *token.address(),
        side: OrderQuoteSide::Sell {
            sell_amount: SellAmount::BeforeFee {
                value: NonZeroU256::try_from(quote_sell_amount).unwrap(),
            },
        },
        ..Default::default()
    };
    assert!(
        matches!(services.submit_quote(&quote_request).await, Err((reqwest::StatusCode::BAD_REQUEST, response)) if response.contains("SameBuyAndSellToken"))
    );
}
