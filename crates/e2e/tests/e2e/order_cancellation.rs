use {
    crate::{
        onchain_components::{deploy_token_with_weth_uniswap_pool, to_wei, WethPoolConfig},
        services::{solvable_orders, wait_for_condition, API_HOST},
        tx,
    },
    ethcontract::{
        prelude::{Account, PrivateKey, U256},
        transaction::TransactionBuilder,
    },
    model::{
        app_id::AppId,
        order::{
            CancellationPayload,
            Order,
            OrderBuilder,
            OrderCancellation,
            OrderCancellations,
            OrderStatus,
            OrderUid,
            SignedOrderCancellations,
        },
        quote::{OrderQuoteRequest, OrderQuoteResponse, OrderQuoteSide, SellAmount},
        signature::{EcdsaSignature, EcdsaSigningScheme},
    },
    secp256k1::SecretKey,
    shared::ethrpc::Web3,
    std::time::Duration,
    web3::signing::SecretKeyRef,
};

const TRADER_PK: [u8; 32] = [1; 32];
const SOLVER_PK: [u8; 32] = [2; 32];

const ORDER_PLACEMENT_ENDPOINT: &str = "/api/v1/orders/";
const QUOTE_ENDPOINT: &str = "/api/v1/quote/";

#[tokio::test]
#[ignore]
async fn local_node_order_cancellation() {
    crate::local_node::test(order_cancellation).await;
}

async fn order_cancellation(web3: Web3) {
    shared::tracing::initialize_reentrant(
        "e2e=debug,orderbook=debug,solver=debug,autopilot=debug,\
         orderbook::api::request_summary=off",
    );
    shared::exit_process_on_panic::set_panic_hook();

    crate::services::clear_database().await;
    let contracts = crate::deploy::deploy(&web3).await.expect("deploy");

    let solver = Account::Offline(PrivateKey::from_raw(SOLVER_PK).unwrap(), None);
    let trader = Account::Offline(PrivateKey::from_raw(TRADER_PK).unwrap(), None);
    for account in [&trader, &solver] {
        TransactionBuilder::new(web3.clone())
            .value(to_wei(1))
            .to(account.address())
            .send()
            .await
            .unwrap();
    }

    contracts
        .gp_authenticator
        .add_solver(solver.address())
        .send()
        .await
        .unwrap();

    // Create & mint tokens to trade, pools for fee connections
    let token = deploy_token_with_weth_uniswap_pool(
        &web3,
        &contracts,
        WethPoolConfig {
            token_amount: to_wei(1000),
            weth_amount: to_wei(1000),
        },
    )
    .await;
    token.mint(trader.address(), to_wei(10)).await;
    let token = token.contract;

    let weth = contracts.weth.clone();

    // Approve GPv2 for trading
    tx!(trader, token.approve(contracts.allowance, to_wei(10)));

    crate::services::start_autopilot(&contracts, &[]);
    crate::services::start_api(&contracts, &[]);
    crate::services::wait_for_api_to_come_up().await;

    let client = reqwest::Client::default();

    let place_order = |salt: u8| {
        let client = &client;
        let request = OrderQuoteRequest {
            from: trader.address(),
            sell_token: token.address(),
            buy_token: weth.address(),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::AfterFee { value: to_wei(1) },
            },
            app_data: AppId([salt; 32]),
            ..Default::default()
        };
        async move {
            let quote = client
                .post(&format!("{API_HOST}{QUOTE_ENDPOINT}"))
                .json(&request)
                .send()
                .await
                .unwrap()
                .json::<OrderQuoteResponse>()
                .await
                .unwrap()
                .quote;

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
                    &contracts.domain_separator,
                    SecretKeyRef::from(&SecretKey::from_slice(&TRADER_PK).unwrap()),
                )
                .build()
                .into_order_creation();

            let placement = client
                .post(&format!("{API_HOST}{ORDER_PLACEMENT_ENDPOINT}"))
                .json(&order)
                .send()
                .await
                .unwrap();

            assert_eq!(placement.status(), 201);

            placement.json::<OrderUid>().await.unwrap()
        }
    };

    let cancel_order = |order_uid: OrderUid| {
        let client = &client;
        let cancellation = OrderCancellation::for_order(
            order_uid,
            &contracts.domain_separator,
            SecretKeyRef::from(&SecretKey::from_slice(&TRADER_PK).unwrap()),
        );

        async move {
            let cancellation = client
                .delete(&format!("{API_HOST}{ORDER_PLACEMENT_ENDPOINT}{order_uid}"))
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
        let client = &client;
        let cancellations = OrderCancellations { order_uids };
        let signing_scheme = EcdsaSigningScheme::Eip712;
        let signature = EcdsaSignature::sign(
            EcdsaSigningScheme::Eip712,
            &contracts.domain_separator,
            &cancellations.hash_struct(),
            SecretKeyRef::from(&SecretKey::from_slice(&TRADER_PK).unwrap()),
        );

        let signed_cancellations = SignedOrderCancellations {
            data: cancellations,
            signature,
            signing_scheme,
        };

        async move {
            let cancellation = client
                .delete(&format!("{API_HOST}{ORDER_PLACEMENT_ENDPOINT}"))
                .json(&signed_cancellations)
                .send()
                .await
                .unwrap();

            assert_eq!(cancellation.status(), 200);
        }
    };

    let get_order = |order_uid: OrderUid| {
        let client = &client;
        async move {
            client
                .get(&format!("{API_HOST}{ORDER_PLACEMENT_ENDPOINT}{order_uid}"))
                .send()
                .await
                .unwrap()
                .json::<Order>()
                .await
                .unwrap()
        }
    };

    // Place 3 orders.
    let order_uids = vec![
        place_order(0).await,
        place_order(1).await,
        place_order(2).await,
    ];
    wait_for_condition(Duration::from_secs(10), || async {
        solvable_orders().await.unwrap() == 3
    })
    .await
    .unwrap();
    for order_uid in &order_uids {
        assert_eq!(
            get_order(*order_uid).await.metadata.status,
            OrderStatus::Open,
        );
    }

    // Cancel one of them.
    cancel_order(order_uids[0]).await;
    wait_for_condition(Duration::from_secs(10), || async {
        solvable_orders().await.unwrap() == 2
    })
    .await
    .unwrap();
    assert_eq!(
        get_order(order_uids[0]).await.metadata.status,
        OrderStatus::Cancelled,
    );

    // Cancel the other two.
    cancel_orders(vec![order_uids[1], order_uids[2]]).await;
    wait_for_condition(Duration::from_secs(10), || async {
        solvable_orders().await.unwrap() == 0
    })
    .await
    .unwrap();
    assert_eq!(
        get_order(order_uids[1]).await.metadata.status,
        OrderStatus::Cancelled,
    );
    assert_eq!(
        get_order(order_uids[2]).await.metadata.status,
        OrderStatus::Cancelled,
    );
}
