use {
    contracts::ERC20,
    e2e::{
        nodes::forked_node::ForkedNodeApi,
        setup::{
            run_forked_test_with_block_number,
            to_wei,
            to_wei_with_exp,
            wait_for_condition,
            OnchainComponents,
            Services,
            TIMEOUT,
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
async fn forked_node_mainnet_single_flashloan_encoding_order() {
    run_forked_test_with_block_number(
        forked_mainnet_single_flashloan_encoding_test,
        std::env::var("FORK_URL_MAINNET")
            .expect("FORK_URL_MAINNET must be set to run forked tests"),
        21874126,
    )
    .await;
}

async fn forked_mainnet_single_flashloan_encoding_test(web3: Web3) {
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
        buy_amount: to_wei_with_exp(500, 6),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        app_data,
        // Receiver is flashloan wrapper, so that borrowed funds can be returned to the lender
        receiver: Some(onchain.contracts().flashloan_wrapper.address()),
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
