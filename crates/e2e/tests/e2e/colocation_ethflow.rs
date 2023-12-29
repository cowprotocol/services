use {
    crate::ethflow::{EthFlowOrderOnchainStatus, EthFlowTradeIntent, ExtendedEthFlowOrder},
    autopilot::database::onchain_order_events::ethflow_events::WRAP_ALL_SELECTOR,
    contracts::ERC20Mintable,
    e2e::setup::{colocation::SolverEngine, *},
    ethcontract::{Account, H160, U256},
    ethrpc::{current_block::timestamp_of_current_block_in_seconds, Web3},
    model::{
        order::{EthflowData, OnchainOrderData, Order, OrderClass, OrderUid},
        quote::{OrderQuoteRequest, OrderQuoteResponse, OrderQuoteSide},
        trade::Trade,
    },
    reqwest::Client,
};

const DAI_PER_ETH: u32 = 1_000;

#[tokio::test]
#[ignore]
async fn local_node_eth_flow() {
    run_test(eth_flow_tx).await;
}

async fn eth_flow_tx(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(to_wei(2)).await;
    let [trader] = onchain.make_accounts(to_wei(2)).await;

    // Create token with Uniswap pool for price estimation
    let [dai] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(DAI_PER_ETH * 1_000), to_wei(1_000))
        .await;

    // Get a quote from the services
    let buy_token = dai.address();
    let receiver = H160([0x42; 20]);
    let sell_amount = to_wei(1);
    let intent = EthFlowTradeIntent {
        sell_amount,
        buy_token,
        receiver,
    };

    let solver_endpoint = colocation::start_solver(onchain.contracts().weth.address()).await;
    colocation::start_driver(
        onchain.contracts(),
        vec![SolverEngine {
            name: "test_solver".into(),
            account: solver,
            endpoint: solver_endpoint,
        }],
    );

    let services = Services::new(onchain.contracts()).await;
    services.start_autopilot(vec![
        "--drivers=test_solver|http://localhost:11088/test_solver".to_string(),
    ]);
    services.start_api(vec![]).await;

    let quote: OrderQuoteResponse = test_submit_quote(
        &services,
        &intent.to_quote_request(&onchain.contracts().ethflow, &onchain.contracts().weth),
    )
    .await;

    let valid_to = chrono::offset::Utc::now().timestamp() as u32
        + timestamp_of_current_block_in_seconds(&web3).await.unwrap()
        + 3600;
    let ethflow_order =
        ExtendedEthFlowOrder::from_quote(&quote, valid_to).include_slippage_bps(300);

    sumbit_order(&ethflow_order, trader.account(), onchain.contracts()).await;

    test_order_availability_in_api(
        &services,
        &ethflow_order,
        &trader.address(),
        onchain.contracts(),
    )
    .await;

    tracing::info!("waiting for trade");
    wait_for_condition(TIMEOUT, || async { services.solvable_orders().await == 1 })
        .await
        .unwrap();

    test_order_was_settled(&services, &ethflow_order, &web3).await;

    test_trade_availability_in_api(
        services.client(),
        &ethflow_order,
        &trader.address(),
        onchain.contracts(),
    )
    .await;
}

async fn test_submit_quote(
    services: &Services<'_>,
    quote: &OrderQuoteRequest,
) -> OrderQuoteResponse {
    let response = services.submit_quote(quote).await.unwrap();

    assert!(response.id.is_some());
    // Ideally the fee would be nonzero, but this is not the case in the test
    // environment assert_ne!(response.quote.fee_amount, 0.into());
    // Amount is reasonable (±10% from real price)
    let approx_output: U256 = response.quote.sell_amount * DAI_PER_ETH;
    assert!(response.quote.buy_amount.gt(&(approx_output * 9u64 / 10)));
    assert!(response.quote.buy_amount.lt(&(approx_output * 11u64 / 10)));

    let OrderQuoteSide::Sell {
        sell_amount:
            model::quote::SellAmount::AfterFee {
                value: sell_amount_after_fees,
            },
    } = quote.side
    else {
        panic!("untested!");
    };

    assert_eq!(response.quote.sell_amount, sell_amount_after_fees.get());

    response
}

async fn sumbit_order(ethflow_order: &ExtendedEthFlowOrder, user: &Account, contracts: &Contracts) {
    assert_eq!(
        ethflow_order.status(contracts).await,
        EthFlowOrderOnchainStatus::Free
    );

    let result = ethflow_order
        .mine_order_creation(user, &contracts.ethflow)
        .await;
    assert_eq!(result.as_receipt().unwrap().status, Some(1.into()));
    assert_eq!(
        ethflow_order.status(contracts).await,
        EthFlowOrderOnchainStatus::Created(user.address(), ethflow_order.0.valid_to)
    );
}

async fn test_order_availability_in_api(
    services: &Services<'_>,
    order: &ExtendedEthFlowOrder,
    owner: &H160,
    contracts: &Contracts,
) {
    tracing::info!("Waiting for order to show up in API.");
    let uid = order.uid(contracts).await;
    let is_available = || async { services.get_order(&uid).await.is_ok() };
    wait_for_condition(TIMEOUT, is_available).await.unwrap();

    test_orders_query(services, order, owner, contracts).await;

    // Api returns eth flow orders for both eth-flow contract address and actual
    // owner
    for address in [owner, &contracts.ethflow.address()] {
        test_account_query(address, services.client(), order, owner, contracts).await;
    }

    wait_for_condition(TIMEOUT, || async { services.solvable_orders().await == 1 })
        .await
        .unwrap();

    test_auction_query(services, order, owner, contracts).await;
}

async fn test_trade_availability_in_api(
    client: &Client,
    order: &ExtendedEthFlowOrder,
    owner: &H160,
    contracts: &Contracts,
) {
    test_trade_query(
        &TradeQuery::ByUid(order.uid(contracts).await),
        client,
        contracts,
    )
    .await;

    // Api returns eth flow orders for both eth-flow contract address and actual
    // owner
    for address in [owner, &contracts.ethflow.address()] {
        test_trade_query(&TradeQuery::ByOwner(*address), client, contracts).await;
    }
}

async fn test_order_was_settled(
    services: &Services<'_>,
    ethflow_order: &ExtendedEthFlowOrder,
    web3: &Web3,
) {
    let auction_is_empty = || async { services.solvable_orders().await == 0 };
    wait_for_condition(TIMEOUT, auction_is_empty).await.unwrap();

    let buy_token = ERC20Mintable::at(web3, ethflow_order.0.buy_token);
    let receiver_buy_token_balance = buy_token
        .balance_of(ethflow_order.0.receiver)
        .call()
        .await
        .expect("Unable to get token balance");
    assert!(receiver_buy_token_balance >= ethflow_order.0.buy_amount);
}

async fn test_orders_query(
    services: &Services<'_>,
    order: &ExtendedEthFlowOrder,
    owner: &H160,
    contracts: &Contracts,
) {
    let response = services
        .get_order(&order.uid(contracts).await)
        .await
        .unwrap();
    test_order_parameters(&response, order, owner, contracts).await;
}

async fn test_account_query(
    queried_account: &H160,
    client: &Client,
    order: &ExtendedEthFlowOrder,
    owner: &H160,
    contracts: &Contracts,
) {
    let query = client
        .get(&format!(
            "{API_HOST}{ACCOUNT_ENDPOINT}/{queried_account:?}/orders",
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(query.status(), 200);
    let response = query.json::<Vec<Order>>().await.unwrap();
    assert_eq!(response.len(), 1);
    test_order_parameters(&response[0], order, owner, contracts).await;
}

async fn test_auction_query(
    services: &Services<'_>,
    order: &ExtendedEthFlowOrder,
    owner: &H160,
    contracts: &Contracts,
) {
    let response = services.get_auction().await;
    assert_eq!(response.auction.orders.len(), 1);
    test_order_parameters(&response.auction.orders[0], order, owner, contracts).await;
}

enum TradeQuery {
    ByUid(OrderUid),
    ByOwner(H160),
}

async fn test_trade_query(query_type: &TradeQuery, client: &Client, contracts: &Contracts) {
    let query = client
        .get(&format!("{API_HOST}{TRADES_ENDPOINT}",))
        .query(&[match query_type {
            TradeQuery::ByUid(uid) => ("orderUid", format!("{uid:?}")),
            TradeQuery::ByOwner(owner) => ("owner", format!("{owner:?}")),
        }])
        .send()
        .await
        .unwrap();
    assert_eq!(query.status(), 200);
    let response = query.json::<Vec<Trade>>().await.unwrap();
    assert_eq!(response.len(), 1);

    // Expected values from actual EIP1271 order instead of eth-flow order
    assert_eq!(response[0].owner, contracts.ethflow.address());
    assert_eq!(response[0].sell_token, contracts.weth.address());
}

async fn test_order_parameters(
    response: &Order,
    order: &ExtendedEthFlowOrder,
    owner: &H160,
    contracts: &Contracts,
) {
    // Expected values from actual EIP1271 order instead of eth-flow order
    assert_eq!(response.data.valid_to, u32::MAX);
    assert_eq!(response.metadata.owner, contracts.ethflow.address());
    assert_eq!(response.data.sell_token, contracts.weth.address());

    // Specific parameters return the missing values
    assert_eq!(
        response.metadata.ethflow_data,
        Some(EthflowData {
            user_valid_to: order.0.valid_to as i64,
            refund_tx_hash: None,
        })
    );
    assert_eq!(
        response.metadata.onchain_order_data,
        Some(OnchainOrderData {
            sender: *owner,
            placement_error: None,
        })
    );

    assert_eq!(response.metadata.class, OrderClass::Market);

    assert!(order
        .is_valid_cowswap_signature(&response.signature, contracts)
        .await
        .is_ok());

    // Requires wrapping first
    assert_eq!(response.interactions.pre.len(), 1);
    assert_eq!(
        response.interactions.pre[0].target,
        contracts.ethflow.address()
    );
    assert_eq!(response.interactions.pre[0].call_data, WRAP_ALL_SELECTOR);
}
