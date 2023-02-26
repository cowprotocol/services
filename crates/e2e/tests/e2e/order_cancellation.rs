use {
    crate::helpers::*,
    ethcontract::prelude::U256,
    model::{
        app_id::AppId,
        order::{
            CancellationPayload,
            OrderBuilder,
            OrderCancellation,
            OrderCancellations,
            OrderStatus,
            OrderUid,
            SignedOrderCancellations,
        },
        quote::{OrderQuoteRequest, OrderQuoteSide, SellAmount},
        signature::{EcdsaSignature, EcdsaSigningScheme},
    },
    secp256k1::SecretKey,
    shared::ethrpc::Web3,
    std::time::Duration,
    web3::signing::SecretKeyRef,
};

#[tokio::test]
#[ignore]
async fn local_node_order_cancellation() {
    crate::local_node::test(order_cancellation).await;
}

async fn order_cancellation(web3: Web3) {
    init().await;

    let mut onchain = OnchainComponents::deploy(web3).await;

    let [_] = onchain.make_solvers(to_wei(1)).await;
    let [trader] = onchain.make_accounts(to_wei(1)).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_pools(to_wei(1_000), to_wei(1_000))
        .await;

    token.mint(trader.address(), to_wei(10)).await;

    // Approve GPv2 for trading
    tx!(
        trader.account(),
        token.approve(onchain.contracts().allowance, to_wei(10))
    );

    let services = Services::new(onchain.contracts()).await;
    services.start_autopilot(vec![]);
    services.start_api(vec![]).await;

    let place_order = |salt: u8| {
        let services = &services;
        let onchain = &onchain;
        let trader = &trader;

        let request = OrderQuoteRequest {
            from: trader.address(),
            sell_token: token.address(),
            buy_token: onchain.contracts().weth.address(),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::AfterFee { value: to_wei(1) },
            },
            app_data: AppId([salt; 32]),
            ..Default::default()
        };
        async move {
            let quote = services.submit_quote(&request).await.unwrap().quote;

            let order = OrderBuilder::default()
                .with_kind(quote.kind)
                .with_sell_token(quote.sell_token)
                .with_sell_amount(quote.sell_amount)
                .with_fee_amount(quote.fee_amount)
                .with_buy_token(quote.buy_token)
                .with_buy_amount((quote.buy_amount * 99) / 100)
                .with_valid_to(quote.valid_to)
                .with_app_data(quote.app_data.0)
                .sign_with(
                    EcdsaSigningScheme::Eip712,
                    &onchain.contracts().domain_separator,
                    SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
                )
                .build()
                .into_order_creation();
            services.create_order(&order).await.unwrap()
        }
    };

    let cancel_order = |order_uid: OrderUid| {
        let client = services.client();
        let cancellation = OrderCancellation::for_order(
            order_uid,
            &onchain.contracts().domain_separator,
            SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
        );

        async move {
            let cancellation = client
                .delete(&format!("{API_HOST}{ORDERS_ENDPOINT}/{order_uid}"))
                .json(&CancellationPayload {
                    signature: cancellation.signature,
                    signing_scheme: cancellation.signing_scheme,
                })
                .send()
                .await
                .unwrap();

            assert_eq!(cancellation.status(), 200);
        }
    };

    let cancel_orders = |order_uids: Vec<OrderUid>| {
        let client = services.client();
        let cancellations = OrderCancellations { order_uids };
        let signing_scheme = EcdsaSigningScheme::Eip712;
        let signature = EcdsaSignature::sign(
            EcdsaSigningScheme::Eip712,
            &onchain.contracts().domain_separator,
            &cancellations.hash_struct(),
            SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
        );

        let signed_cancellations = SignedOrderCancellations {
            data: cancellations,
            signature,
            signing_scheme,
        };

        async move {
            let cancellation = client
                .delete(&format!("{API_HOST}{ORDERS_ENDPOINT}"))
                .json(&signed_cancellations)
                .send()
                .await
                .unwrap();

            assert_eq!(cancellation.status(), 200);
        }
    };

    // Place 3 orders.
    let order_uids = vec![
        place_order(0).await,
        place_order(1).await,
        place_order(2).await,
    ];
    wait_for_condition(Duration::from_secs(10), || async {
        services.get_auction().await.auction.orders.len() == 3
    })
    .await
    .unwrap();
    for order_uid in &order_uids {
        assert_eq!(
            services.get_order(order_uid).await.unwrap().metadata.status,
            OrderStatus::Open,
        );
    }

    // Cancel one of them.
    cancel_order(order_uids[0]).await;
    wait_for_condition(Duration::from_secs(10), || async {
        services.get_auction().await.auction.orders.len() == 2
    })
    .await
    .unwrap();
    assert_eq!(
        services
            .get_order(&order_uids[0])
            .await
            .unwrap()
            .metadata
            .status,
        OrderStatus::Cancelled,
    );

    // Cancel the other two.
    cancel_orders(vec![order_uids[1], order_uids[2]]).await;
    wait_for_condition(Duration::from_secs(10), || async {
        services.get_auction().await.auction.orders.is_empty()
    })
    .await
    .unwrap();
    assert_eq!(
        services
            .get_order(&order_uids[1])
            .await
            .unwrap()
            .metadata
            .status,
        OrderStatus::Cancelled,
    );
    assert_eq!(
        services
            .get_order(&order_uids[2])
            .await
            .unwrap()
            .metadata
            .status,
        OrderStatus::Cancelled,
    );
}
