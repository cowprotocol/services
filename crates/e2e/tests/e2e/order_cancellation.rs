use crate::{
    services::{create_orderbook_api, deploy_mintable_token, to_wei, OrderbookServices, API_HOST},
    tx, tx_value,
};
use ethcontract::prelude::{Account, Address, PrivateKey, U256};
use model::{
    app_id::AppId,
    order::{
        CancellationPayload, Order, OrderBuilder, OrderCancellation, OrderCancellations,
        OrderStatus, OrderUid, SignedOrderCancellations,
    },
    quote::{OrderQuoteRequest, OrderQuoteResponse, OrderQuoteSide, SellAmount},
    signature::{EcdsaSignature, EcdsaSigningScheme},
};
use secp256k1::SecretKey;
use shared::{ethrpc::Web3, http_client::HttpClientFactory, maintenance::Maintaining};
use web3::signing::SecretKeyRef;

const TRADER_PK: [u8; 32] = [1; 32];

const ORDER_PLACEMENT_ENDPOINT: &str = "/api/v1/orders/";
const QUOTE_ENDPOINT: &str = "/api/v1/quote/";

#[tokio::test]
#[ignore]
async fn local_node_order_cancellation() {
    crate::local_node::test(order_cancellation).await;
}

async fn order_cancellation(web3: Web3) {
    shared::tracing::initialize_for_tests("warn,orderbook=debug,shared=debug");
    shared::exit_process_on_panic::set_panic_hook();

    let contracts = crate::deploy::deploy(&web3).await.expect("deploy");

    let accounts: Vec<Address> = web3.eth().accounts().await.expect("get accounts failed");
    let solver_account = Account::Local(accounts[0], None);
    let trader = Account::Offline(PrivateKey::from_raw(TRADER_PK).unwrap(), None);

    // Create & Mint tokens to trade
    let token = deploy_mintable_token(&web3).await;
    tx!(
        solver_account,
        token.mint(solver_account.address(), to_wei(100_000))
    );
    tx!(solver_account, token.mint(trader.address(), to_wei(100)));

    let weth = contracts.weth.clone();
    tx_value!(solver_account, to_wei(100_000), weth.deposit());

    // Create and fund Uniswap pool
    tx!(
        solver_account,
        contracts
            .uniswap_factory
            .create_pair(token.address(), weth.address())
    );
    tx!(
        solver_account,
        token.approve(contracts.uniswap_router.address(), to_wei(100_000))
    );
    tx!(
        solver_account,
        weth.approve(contracts.uniswap_router.address(), to_wei(100_000))
    );
    tx!(
        solver_account,
        contracts.uniswap_router.add_liquidity(
            token.address(),
            weth.address(),
            to_wei(100_000),
            to_wei(100_000),
            0_u64.into(),
            0_u64.into(),
            solver_account.address(),
            U256::max_value(),
        )
    );

    // Approve GPv2 for trading
    tx!(trader, token.approve(contracts.allowance, to_wei(100)));

    let OrderbookServices {
        maintenance,
        solvable_orders_cache,
        ..
    } = OrderbookServices::new(&web3, &contracts, false).await;

    let http_factory = HttpClientFactory::default();
    let client = http_factory.create();

    let place_order = |salt: u8| {
        let client = &client;
        let request = OrderQuoteRequest {
            from: trader.address(),
            sell_token: token.address(),
            buy_token: weth.address(),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::AfterFee { value: to_wei(10) },
            },
            app_data: AppId([salt; 32]),
            ..Default::default()
        };
        async move {
            let quote = client
                .post(&format!("{}{}", API_HOST, QUOTE_ENDPOINT))
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
                .post(&format!("{}{}", API_HOST, ORDER_PLACEMENT_ENDPOINT))
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
                .delete(&format!(
                    "{}{}{}",
                    API_HOST, ORDER_PLACEMENT_ENDPOINT, order_uid
                ))
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
                .delete(&format!("{}{}", API_HOST, ORDER_PLACEMENT_ENDPOINT))
                .json(&signed_cancellations)
                .send()
                .await
                .unwrap();

            assert_eq!(cancellation.status(), 200);
        }
    };

    let get_auction = || async {
        maintenance.run_maintenance().await.unwrap();
        solvable_orders_cache.update(0).await.unwrap();
        create_orderbook_api().get_auction().await.unwrap().auction
    };

    let get_order = |order_uid: OrderUid| {
        let client = &client;
        async move {
            client
                .get(&format!(
                    "{}{}{}",
                    API_HOST, ORDER_PLACEMENT_ENDPOINT, order_uid
                ))
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
    assert_eq!(get_auction().await.orders.len(), 3);
    for order_uid in &order_uids {
        assert_eq!(
            get_order(*order_uid).await.metadata.status,
            OrderStatus::Open,
        );
    }

    // Cancel one of them.
    cancel_order(order_uids[0]).await;
    assert_eq!(get_auction().await.orders.len(), 2);
    assert_eq!(
        get_order(order_uids[0]).await.metadata.status,
        OrderStatus::Cancelled,
    );

    // Cancel the other two.
    cancel_orders(vec![order_uids[1], order_uids[2]]).await;
    assert_eq!(get_auction().await.orders.len(), 0);
    assert_eq!(
        get_order(order_uids[1]).await.metadata.status,
        OrderStatus::Cancelled,
    );
    assert_eq!(
        get_order(order_uids[2]).await.metadata.status,
        OrderStatus::Cancelled,
    );
}
