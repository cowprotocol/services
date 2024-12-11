use {
    driver::domain::eth::NonZeroU256,
    e2e::{nodes::local_node::TestNodeApi, setup::*, tx, tx_value},
    ethcontract::U256,
    model::{
        order::{OrderCreation, OrderKind},
        quote::{OrderQuoteRequest, OrderQuoteSide, SellAmount},
        signature::EcdsaSigningScheme,
    },
    secp256k1::SecretKey,
    shared::ethrpc::Web3,
    std::ops::DerefMut,
    web3::signing::SecretKeyRef,
};

#[tokio::test]
#[ignore]
async fn local_node_test() {
    run_test(place_order_with_quote).await;
}

async fn place_order_with_quote(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(to_wei(10)).await;
    let [trader] = onchain.make_accounts(to_wei(10)).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1_000))
        .await;

    tx!(
        trader.account(),
        onchain
            .contracts()
            .weth
            .approve(onchain.contracts().allowance, to_wei(3))
    );
    tx_value!(
        trader.account(),
        to_wei(3),
        onchain.contracts().weth.deposit()
    );

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    services.start_protocol(solver.clone()).await;

    // Disable auto-mine so we don't accidentally mine a settlement
    web3.api::<TestNodeApi<_>>()
        .disable_automine()
        .await
        .expect("Must be able to disable automine");

    tracing::info!("Quoting");
    let quote_sell_amount = to_wei(1);
    let quote_request = OrderQuoteRequest {
        from: trader.address(),
        sell_token: onchain.contracts().weth.address(),
        buy_token: token.address(),
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

    let quote_metadata =
        crate::database::quote_metadata(services.db(), quote_response.id.unwrap()).await;
    assert!(quote_metadata.is_some());
    tracing::debug!(?quote_metadata);

    tracing::info!("Placing order");
    let balance = token.balance_of(trader.address()).call().await.unwrap();
    assert_eq!(balance, 0.into());
    let order = OrderCreation {
        quote_id: quote_response.id,
        sell_token: onchain.contracts().weth.address(),
        sell_amount: quote_sell_amount,
        buy_token: token.address(),
        buy_amount: quote_response.quote.buy_amount,
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );
    let order_uid = services.create_order(&order).await.unwrap();

    tracing::info!("Order quote verification");
    let order_quote = database::orders::read_quote(
        services.db().acquire().await.unwrap().deref_mut(),
        &database::byte_array::ByteArray(order_uid.0),
    )
    .await
    .unwrap();
    assert!(order_quote.is_some());
    // compare quote metadata and order quote metadata
    let order_quote_metadata = order_quote.unwrap().metadata;
    assert_eq!(quote_metadata.unwrap().0, order_quote_metadata);
}
