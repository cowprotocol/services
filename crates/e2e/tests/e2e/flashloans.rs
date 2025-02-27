use {
    contracts::ERC20,
    e2e::{
        nodes::forked_node::ForkedNodeApi,
        setup::{
            OnchainComponents,
            Services,
            TIMEOUT,
            run_forked_test_with_block_number,
            to_wei,
            to_wei_with_exp,
            wait_for_condition,
        },
        tx,
    },
    ethcontract::{H160, U256},
    ethrpc::Web3,
    model::{
        order::{OrderCreation, OrderCreationAppData, OrderKind},
        quote::{OrderQuoteRequest, OrderQuoteSide, SellAmount},
        signature::EcdsaSigningScheme,
    },
    secp256k1::SecretKey,
    web3::signing::SecretKeyRef,
};

#[tokio::test]
#[ignore]
async fn forked_node_mainnet_single_flashloan_encoding_maker() {
    run_forked_test_with_block_number(
        forked_mainnet_single_flashloan_encoding_maker_test,
        std::env::var("FORK_URL_MAINNET")
            .expect("FORK_URL_MAINNET must be set to run forked tests"),
        21874126,
    )
    .await;
}

#[tokio::test]
#[ignore]
async fn forked_node_mainnet_single_flashloan_encoding_aave() {
    run_forked_test_with_block_number(
        forked_mainnet_single_flashloan_encoding_aave_test,
        std::env::var("FORK_URL_MAINNET")
            .expect("FORK_URL_MAINNET must be set to run forked tests"),
        21874126,
    )
    .await;
}

async fn forked_mainnet_single_flashloan_encoding_maker_test(web3: Web3) {
    let mut onchain = OnchainComponents::deployed(web3.clone()).await;
    let forked_node_api = web3.api::<ForkedNodeApi<_>>();

    let [solver] = onchain.make_solvers_forked(to_wei(1)).await;

    let [trader] = onchain.make_accounts(to_wei(1)).await;

    let token_usdc = ERC20::at(
        &web3,
        "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
            .parse()
            .unwrap(),
    );

    let token_dai = ERC20::at(
        &web3,
        "0x6B175474E89094C44Da98b954EedeAC495271d0F"
            .parse()
            .unwrap(),
    );

    // find some USDC available onchain
    const USDC_WHALE_MAINNET: H160 = H160(hex_literal::hex!(
        "28c6c06298d514db089934071355e5743bf21d60"
    ));
    // Give trader some USDC
    let usdc_whale = forked_node_api
        .impersonate(&USDC_WHALE_MAINNET)
        .await
        .unwrap();
    tx!(
        usdc_whale,
        token_usdc.transfer(trader.address(), to_wei_with_exp(1000, 6))
    );

    // Approve GPv2 for trading
    tx!(
        trader.account(),
        token_usdc.approve(onchain.contracts().allowance, to_wei_with_exp(1000, 6))
    );

    // Place Orders
    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    onchain.mint_block().await;

    // App data with flashloan
    let app_data = format!(
        r#"{{
        "metadata": {{
            "flashloan": {{
                "lender": "0x60744434d6339a6B27d73d9Eda62b6F66a0a04FA",
                "borrower": "{:?}",
                "token": "0x6B175474E89094C44Da98b954EedeAC495271d0F",
                "amount": "900000000000000000000"
            }}
        }}
    }}"#,
        trader.address()
    );

    let app_data = OrderCreationAppData::Full {
        full: app_data.to_string(),
    };

    let order = OrderCreation {
        sell_token: token_usdc.address(),
        sell_amount: to_wei_with_exp(1000, 6),
        buy_token: token_dai.address(),
        buy_amount: to_wei_with_exp(900, 18), // equal to flashloan amount
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Buy,
        app_data,
        partially_fillable: false,
        // Receiver is always the settlement contract, so driver will have to manually send funds to
        // solver wrapper (flashloan borrower)
        receiver: Some(onchain.contracts().gp_settlement.address()),
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );

    // Warm up co-located driver by quoting the order (otherwise placing an order
    // may time out)
    let _ = services
        .submit_quote(&OrderQuoteRequest {
            sell_token: token_usdc.address(),
            buy_token: token_dai.address(),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: to_wei_with_exp(1000, 6).try_into().unwrap(),
                },
            },
            ..Default::default()
        })
        .await;
    let order_id = services.create_order(&order).await.unwrap();

    // Drive solution
    tracing::info!("Waiting for trade.");

    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;

        let executed_fee = services
            .get_order(&order_id)
            .await
            .unwrap()
            .metadata
            .executed_fee;
        executed_fee > 0.into()

        // TODO balances
    })
    .await
    .unwrap();
}

async fn forked_mainnet_single_flashloan_encoding_aave_test(web3: Web3) {
    let mut onchain = OnchainComponents::deployed(web3.clone()).await;
    let forked_node_api = web3.api::<ForkedNodeApi<_>>();

    let [solver] = onchain.make_solvers_forked(to_wei(1)).await;

    let [trader] = onchain.make_accounts(to_wei(1)).await;

    let token_usdc = ERC20::at(
        &web3,
        "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
            .parse()
            .unwrap(),
    );

    let token_weth = ERC20::at(
        &web3,
        "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
            .parse()
            .unwrap(),
    );

    // find some USDC available onchain
    const USDC_WHALE_MAINNET: H160 = H160(hex_literal::hex!(
        "28c6c06298d514db089934071355e5743bf21d60"
    ));
    // Give trader some USDC
    let usdc_whale = forked_node_api
        .impersonate(&USDC_WHALE_MAINNET)
        .await
        .unwrap();
    tx!(
        usdc_whale,
        token_usdc.transfer(trader.address(), to_wei_with_exp(50000, 6))
    );

    // Approve GPv2 for trading
    tx!(
        trader.account(),
        token_usdc.approve(onchain.contracts().allowance, to_wei_with_exp(50000, 6))
    );

    // Place Orders
    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    onchain.mint_block().await;

    // App data with flashloan
    let app_data = format!(
        r#"{{
        "metadata": {{
            "flashloan": {{
                "lender": "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2",
                "borrower": "{:?}",
                "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                "amount": "5000000000000000000"
            }}
        }}
    }}"#,
        trader.address()
    );

    let app_data = OrderCreationAppData::Full {
        full: app_data.to_string(),
    };

    let order = OrderCreation {
        sell_token: token_usdc.address(),
        sell_amount: to_wei_with_exp(50000, 6),
        buy_token: token_weth.address(),
        buy_amount: U256::from(5005000000000000000u128), // equal to flashloan amount + 0.1% fee
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Buy,
        app_data,
        partially_fillable: false,
        // Receiver is always the settlement contract, so driver will have to manually send funds to
        // solver wrapper (flashloan borrower)
        receiver: Some(onchain.contracts().gp_settlement.address()),
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );

    // Warm up co-located driver by quoting the order (otherwise placing an order
    // may time out)
    let _ = services
        .submit_quote(&OrderQuoteRequest {
            sell_token: token_usdc.address(),
            buy_token: token_weth.address(),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: to_wei_with_exp(50000, 6).try_into().unwrap(),
                },
            },
            ..Default::default()
        })
        .await;
    let order_id = services.create_order(&order).await.unwrap();

    // Drive solution
    tracing::info!("Waiting for trade.");

    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;

        let executed_fee = services
            .get_order(&order_id)
            .await
            .unwrap()
            .metadata
            .executed_fee;
        executed_fee > 0.into()

        // TODO balances
    })
    .await
    .unwrap();
}
