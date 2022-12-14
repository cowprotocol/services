use crate::{
    deploy::Contracts,
    local_node::{AccountAssigner, TestNodeApi},
    onchain_components::{
        deploy_token_with_weth_uniswap_pool, to_wei, MintableToken, WethPoolConfig,
    },
    services::{
        create_orderbook_api, setup_naive_solver_uniswapv2_driver, wait_for_solvable_orders,
        OrderbookServices, API_HOST,
    },
};
use anyhow::bail;
use autopilot::database::onchain_order_events::ethflow_events::WRAP_ALL_SELECTOR;
use chrono::{DateTime, NaiveDateTime, Utc};
use contracts::{CoWSwapEthFlow, ERC20Mintable, WETH9};
use ethcontract::{transaction::TransactionResult, Account, Bytes, H160, H256, U256};
use hex_literal::hex;
use model::{
    app_id::AppId,
    auction::AuctionWithId,
    order::{
        BuyTokenDestination, EthflowData, Order, OrderBuilder, OrderClass, OrderKind, OrderUid,
        SellTokenSource,
    },
    quote::{
        OrderQuoteRequest, OrderQuoteResponse, OrderQuoteSide, PriceQuality, QuoteSigningScheme,
        Validity,
    },
    signature::{hashed_eip712_message, Signature},
    trade::Trade,
    DomainSeparator,
};
use refunder::{
    ethflow_order::EthflowOrder,
    refund_service::{INVALIDATED_OWNER, NO_OWNER},
};
use reqwest::Client;
use shared::{
    current_block::timestamp_of_current_block_in_seconds, ethrpc::Web3,
    http_client::HttpClientFactory, maintenance::Maintaining,
    signature_validator::check_erc1271_result,
};
const ACCOUNT_ENDPOINT: &str = "/api/v1/account";
const AUCTION_ENDPOINT: &str = "/api/v1/auction";
const ORDERS_ENDPOINT: &str = "/api/v1/orders";
const QUOTE_ENDPOINT: &str = "/api/v1/quote";
const TRADES_ENDPOINT: &str = "/api/v1/trades";

const DAI_PER_ETH: u32 = 1000;

#[tokio::test]
#[ignore]
async fn local_node_eth_flow() {
    crate::local_node::test(eth_flow_tx).await;
}

#[tokio::test]
#[ignore]
async fn local_node_eth_flow_indexing_after_refund() {
    crate::local_node::test(eth_flow_indexing_after_refund).await;
}

async fn eth_flow_tx(web3: Web3) {
    shared::tracing::initialize_for_tests(
        "warn,orderbook=debug,orderbook=debug,solver=debug,autopilot=debug",
    );
    shared::exit_process_on_panic::set_panic_hook();
    let contracts = crate::deploy::deploy(&web3).await.expect("deploy");

    let mut accounts = AccountAssigner::new(&web3).await;
    let trader = accounts.assign_free_account();
    let solver = accounts.default_deployer; // is a solver by default

    // Create token with Uniswap pool for price estimation
    let MintableToken { contract: dai, .. } = deploy_token_with_weth_uniswap_pool(
        &web3,
        &contracts,
        WethPoolConfig {
            // 1 ETH ≈ 1k DAI
            token_amount: to_wei(DAI_PER_ETH * 1_000),
            weth_amount: to_wei(1_000),
        },
    )
    .await;

    let OrderbookServices {
        maintenance,
        block_stream,
        solvable_orders_cache,
        base_tokens,
        ..
    } = OrderbookServices::new(&web3, &contracts, false).await;
    let http_factory = HttpClientFactory::default();
    let client = http_factory.create();

    // Get a quote from the services
    let buy_token = dai.address();
    let receiver = H160([0x42; 20]);
    let sell_amount = to_wei(1);
    let intent = EthFlowTradeIntent {
        sell_amount,
        buy_token,
        receiver,
    };

    let quote: OrderQuoteResponse = submit_quote(
        &intent.to_quote_request(&contracts.ethflow, &contracts.weth),
        &client,
    )
    .await;

    let valid_to = chrono::offset::Utc::now().timestamp() as u32 + 3600;
    let ethflow_order =
        ExtendedEthFlowOrder::from_quote(&quote, valid_to).include_slippage_bps(300);

    sumbit_order(&ethflow_order, &trader, &contracts).await;

    // Automatically pick up onchain order without any API request
    maintenance.run_maintenance().await.unwrap();

    test_order_availability_in_api(&client, &ethflow_order, &trader.address(), &contracts).await;

    let mut driver =
        setup_naive_solver_uniswapv2_driver(&web3, &contracts, base_tokens, block_stream, solver)
            .await;
    driver.single_run().await.unwrap();

    // Update orderbook status
    maintenance.run_maintenance().await.unwrap();
    solvable_orders_cache.update(0).await.unwrap();

    test_order_was_settled(&ethflow_order, &web3).await;

    test_trade_availability_in_api(&client, &ethflow_order, &trader.address(), &contracts).await;
}

async fn eth_flow_indexing_after_refund(web3: Web3) {
    shared::tracing::initialize_for_tests(
        "warn,orderbook=debug,orderbook=debug,solver=debug,autopilot=debug",
    );
    shared::exit_process_on_panic::set_panic_hook();
    let contracts = crate::deploy::deploy(&web3).await.expect("deploy");

    let mut accounts = AccountAssigner::new(&web3).await;
    let trader = accounts.assign_free_account();
    let dummy_trader = accounts.assign_free_account();
    let refunder = accounts.assign_free_account();
    let solver = accounts.default_deployer; // is a solver by default

    // Create token with Uniswap pool for price estimation
    let MintableToken { contract: dai, .. } = deploy_token_with_weth_uniswap_pool(
        &web3,
        &contracts,
        WethPoolConfig {
            // 1 ETH ≈ 1k DAI
            token_amount: to_wei(DAI_PER_ETH * 1_000),
            weth_amount: to_wei(1_000),
        },
    )
    .await;

    let OrderbookServices {
        maintenance,
        block_stream,
        solvable_orders_cache,
        base_tokens,
        ..
    } = OrderbookServices::new(&web3, &contracts, false).await;
    let http_factory = HttpClientFactory::default();
    let client = http_factory.create();

    // Create an order that only exists to be refunded, which triggers an event in the eth-flow contract that is not
    // included in the ABI of `CoWSwapOnchainOrders`.
    let valid_to = timestamp_of_current_block_in_seconds(&web3).await.unwrap() + 60;
    let dummy_order = ExtendedEthFlowOrder::from_quote(
        &submit_quote(
            &(EthFlowTradeIntent {
                sell_amount: 42.into(),
                buy_token: dai.address(),
                receiver: H160([42; 20]),
            })
            .to_quote_request(&contracts.ethflow, &contracts.weth),
            &client,
        )
        .await,
        valid_to,
    )
    .include_slippage_bps(300);
    sumbit_order(&dummy_order, &dummy_trader, &contracts).await;
    web3.api::<TestNodeApi<_>>()
        .set_next_block_timestamp(&DateTime::from_utc(
            NaiveDateTime::from_timestamp(valid_to as i64 + 1, 0),
            Utc,
        ))
        .await
        .expect("Must be able to set block timestamp");
    dummy_order
        .mine_order_invalidation(&refunder, &contracts.ethflow)
        .await;

    // Create the actual order that should be picked up by the services and matched.
    let buy_token = dai.address();
    let receiver = H160([0x42; 20]);
    let sell_amount = to_wei(1);
    let valid_to = timestamp_of_current_block_in_seconds(&web3).await.unwrap() + 60;
    let ethflow_order = ExtendedEthFlowOrder::from_quote(
        &submit_quote(
            &(EthFlowTradeIntent {
                sell_amount,
                buy_token,
                receiver,
            })
            .to_quote_request(&contracts.ethflow, &contracts.weth),
            &client,
        )
        .await,
        valid_to,
    )
    .include_slippage_bps(300);
    sumbit_order(&ethflow_order, &trader, &contracts).await;

    // Automatically pick up onchain order without any API request
    maintenance.run_maintenance().await.unwrap();

    wait_for_solvable_orders(&client, 1).await.unwrap();

    let mut driver =
        setup_naive_solver_uniswapv2_driver(&web3, &contracts, base_tokens, block_stream, solver)
            .await;
    driver.single_run().await.unwrap();

    // Update orderbook status
    maintenance.run_maintenance().await.unwrap();
    solvable_orders_cache.update(0).await.unwrap();

    test_order_was_settled(&ethflow_order, &web3).await;
}

async fn submit_quote(quote: &OrderQuoteRequest, client: &reqwest::Client) -> OrderQuoteResponse {
    let quoting = client
        .post(&format!("{API_HOST}{QUOTE_ENDPOINT}"))
        .json(&quote)
        .send()
        .await
        .unwrap();
    assert_eq!(quoting.status(), 200);
    let response = quoting.json::<OrderQuoteResponse>().await.unwrap();

    assert!(response.id.is_some());
    // Ideally the fee would be nonzero, but this is not the case in the test environment
    //assert_ne!(response.quote.fee_amount, 0.into());
    // Amount is reasonable (±10% from real price)
    let approx_output: U256 = response.quote.sell_amount * DAI_PER_ETH;
    assert!(response.quote.buy_amount.gt(&(approx_output * 9u64 / 10)));
    assert!(response.quote.buy_amount.lt(&(approx_output * 11u64 / 10)));

    if let OrderQuoteSide::Sell {
        sell_amount:
            model::quote::SellAmount::AfterFee {
                value: sell_amount_after_fees,
            },
    } = quote.side
    {
        assert_eq!(response.quote.sell_amount, sell_amount_after_fees);
    } else {
        panic!("Untested")
    };

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
    client: &Client,
    order: &ExtendedEthFlowOrder,
    owner: &H160,
    contracts: &Contracts,
) {
    test_orders_query(client, order, owner, contracts).await;

    // Api returns eth flow orders for both eth-flow contract address and actual owner
    for address in [owner, &contracts.ethflow.address()] {
        test_account_query(address, client, order, owner, contracts).await;
    }

    wait_for_solvable_orders(client, 1).await.unwrap();

    test_auction_query(client, order, owner, contracts).await;
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

    // Api returns eth flow orders for both eth-flow contract address and actual owner
    for address in [owner, &contracts.ethflow.address()] {
        test_trade_query(&TradeQuery::ByOwner(*address), client, contracts).await;
    }
}

async fn test_order_was_settled(ethflow_order: &ExtendedEthFlowOrder, web3: &Web3) {
    let auction = create_orderbook_api().get_auction().await.unwrap();
    assert!(auction.auction.orders.is_empty());

    let buy_token = ERC20Mintable::at(web3, ethflow_order.0.buy_token);
    let receiver_buy_token_balance = buy_token
        .balance_of(ethflow_order.0.receiver)
        .call()
        .await
        .expect("Unable to get token balance");
    assert!(receiver_buy_token_balance >= ethflow_order.0.buy_amount);
}

async fn test_orders_query(
    client: &Client,
    order: &ExtendedEthFlowOrder,
    owner: &H160,
    contracts: &Contracts,
) {
    let query = client
        .get(&format!(
            "{API_HOST}{ORDERS_ENDPOINT}/{}",
            order.uid(contracts).await
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(query.status(), 200);
    let response = query.json::<Order>().await.unwrap();
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
    client: &Client,
    order: &ExtendedEthFlowOrder,
    owner: &H160,
    contracts: &Contracts,
) {
    let query = client
        .get(&format!("{API_HOST}{AUCTION_ENDPOINT}"))
        .send()
        .await
        .unwrap();
    assert_eq!(query.status(), 200);
    let response = query.json::<AuctionWithId>().await.unwrap();
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
            is_refunded: false
        })
    );
    assert_eq!(response.metadata.onchain_user, Some(*owner));

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

pub struct ExtendedEthFlowOrder(pub EthflowOrder);

impl ExtendedEthFlowOrder {
    pub fn from_quote(quote_response: &OrderQuoteResponse, valid_to: u32) -> Self {
        let quote = &quote_response.quote;
        ExtendedEthFlowOrder(EthflowOrder {
            buy_token: quote.buy_token,
            receiver: quote.receiver.expect("eth-flow order without receiver"),
            sell_amount: quote.sell_amount,
            buy_amount: quote.buy_amount,
            app_data: ethcontract::Bytes(quote.app_data.0),
            fee_amount: quote.fee_amount,
            valid_to, // note: valid to in the quote is always unlimited
            partially_fillable: quote.partially_fillable,
            quote_id: quote_response.id.expect("No quote id"),
        })
    }

    fn to_cow_swap_order(&self, ethflow: &CoWSwapEthFlow, weth: &WETH9) -> Order {
        // Each ethflow user order has an order that is representing
        // it as EIP1271 order with a different owner and valid_to
        OrderBuilder::default()
            .with_kind(OrderKind::Sell)
            .with_sell_token(weth.address())
            .with_sell_amount(self.0.sell_amount)
            .with_fee_amount(self.0.fee_amount)
            .with_receiver(Some(self.0.receiver))
            .with_buy_token(self.0.buy_token)
            .with_buy_amount(self.0.buy_amount)
            .with_valid_to(u32::MAX)
            .with_app_data(self.0.app_data.0)
            .with_class(OrderClass::Market) // Eth-flow orders only support market orders at this point in time
            .with_eip1271(ethflow.address(), hex!("").into())
            .build()
    }

    pub fn include_slippage_bps(&self, slippage: u16) -> Self {
        const MAX_BASE_POINT: u16 = 10000;
        if slippage > MAX_BASE_POINT {
            panic!("Slippage must be specified in base points");
        }
        ExtendedEthFlowOrder(EthflowOrder {
            buy_amount: self.0.buy_amount * (MAX_BASE_POINT - slippage) / MAX_BASE_POINT,
            ..self.0
        })
    }

    pub async fn status(&self, contracts: &Contracts) -> EthFlowOrderOnchainStatus {
        contracts
            .ethflow
            .orders(Bytes(self.hash(contracts).await.0))
            .call()
            .await
            .expect("Couldn't fetch order status")
            .into()
    }

    pub async fn is_valid_cowswap_signature(
        &self,
        cowswap_signature: &Signature,
        contracts: &Contracts,
    ) -> anyhow::Result<()> {
        let bytes = match cowswap_signature {
            Signature::Eip1271(bytes) => bytes,
            _ => bail!(
                "Invalid signature type, expected EIP1271, found {:?}",
                cowswap_signature
            ),
        }
        .clone();

        let result = contracts
            .ethflow
            .is_valid_signature(
                Bytes(self.hash(contracts).await.to_fixed_bytes()),
                Bytes(bytes),
            )
            .call()
            .await
            .expect("Couldn't fetch signature validity");

        check_erc1271_result(result)
            .map_err(|err| anyhow::anyhow!("failed signature verification: {:?}", err))
    }

    pub async fn mine_order_creation(
        &self,
        owner: &Account,
        ethflow: &CoWSwapEthFlow,
    ) -> TransactionResult {
        tx_value!(
            owner,
            self.0.sell_amount + self.0.fee_amount,
            ethflow.create_order(self.0.encode())
        )
    }

    pub async fn mine_order_invalidation(
        &self,
        sender: &Account,
        ethflow: &CoWSwapEthFlow,
    ) -> TransactionResult {
        tx!(sender, ethflow.invalidate_order(self.0.encode()))
    }

    async fn hash(&self, contracts: &Contracts) -> H256 {
        let domain_separator = DomainSeparator(
            contracts
                .gp_settlement
                .domain_separator()
                .call()
                .await
                .expect("Couldn't query domain separator")
                .0,
        );
        H256(hashed_eip712_message(
            &domain_separator,
            &self
                .to_cow_swap_order(&contracts.ethflow, &contracts.weth)
                .data
                .hash_struct(),
        ))
    }

    async fn uid(&self, contracts: &Contracts) -> OrderUid {
        let domain_separator = DomainSeparator(
            contracts
                .gp_settlement
                .domain_separator()
                .call()
                .await
                .expect("Couldn't query domain separator")
                .0,
        );
        self.to_cow_swap_order(&contracts.ethflow, &contracts.weth)
            .data
            .uid(&domain_separator, &contracts.ethflow.address())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum EthFlowOrderOnchainStatus {
    Invalidated,
    Created(H160, u32),
    Free,
}

impl From<(H160, u32)> for EthFlowOrderOnchainStatus {
    fn from((owner, valid_to): (H160, u32)) -> Self {
        match owner {
            owner if owner == NO_OWNER => Self::Free,
            owner if owner == INVALIDATED_OWNER => Self::Invalidated,
            _ => Self::Created(owner, valid_to),
        }
    }
}

struct EthFlowTradeIntent {
    sell_amount: U256,
    buy_token: H160,
    receiver: H160,
}

impl EthFlowTradeIntent {
    // How a user trade intent is converted into a quote request by the frontend
    fn to_quote_request(&self, ethflow: &CoWSwapEthFlow, weth: &WETH9) -> OrderQuoteRequest {
        OrderQuoteRequest {
            from: ethflow.address(),
            // Even if the user sells ETH, we request a quote for WETH
            sell_token: weth.address(),
            buy_token: self.buy_token,
            receiver: Some(self.receiver),
            validity: Validity::For(u32::MAX),
            app_data: AppId([0x42; 32]),
            signing_scheme: QuoteSigningScheme::Eip1271 {
                onchain_order: true,
                verification_gas_limit: 0,
            },
            side: OrderQuoteSide::Sell {
                sell_amount: model::quote::SellAmount::AfterFee {
                    value: self.sell_amount,
                },
            },
            buy_token_balance: BuyTokenDestination::Erc20,
            sell_token_balance: SellTokenSource::Erc20,
            partially_fillable: false,
            price_quality: PriceQuality::Optimal,
        }
    }
}
