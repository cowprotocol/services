use {
    alloy::{
        primitives::{Address, B256, Bytes, U256},
        rpc::types::TransactionReceipt,
    },
    anyhow::bail,
    autopilot::database::onchain_order_events::ethflow_events::WRAP_ALL_SELECTOR,
    contracts::alloy::{CoWSwapEthFlow, ERC20Mintable, WETH9},
    database::order_events::OrderEventLabel,
    e2e::setup::{
        ACCOUNT_ENDPOINT,
        API_HOST,
        Contracts,
        OnchainComponents,
        Services,
        TIMEOUT,
        TRADES_ENDPOINT,
        TestAccount,
        run_test,
        wait_for_condition,
    },
    ethrpc::{Web3, alloy::CallBuilderExt, block_stream::timestamp_of_current_block_in_seconds},
    model::{
        DomainSeparator,
        order::{
            BuyTokenDestination,
            EthflowData,
            OnchainOrderData,
            Order,
            OrderBuilder,
            OrderClass,
            OrderCreationAppData,
            OrderKind,
            OrderUid,
            SellTokenSource,
        },
        quote::{
            OrderQuoteRequest,
            OrderQuoteResponse,
            OrderQuoteSide,
            PriceQuality,
            QuoteSigningScheme,
            Validity,
        },
        signature::{Signature, hashed_eip712_message},
        trade::Trade,
    },
    number::{nonzero::NonZeroU256, units::EthUnit},
    refunder::RefundStatus,
    reqwest::Client,
    shared::signature_validator::check_erc1271_result,
};

const DAI_PER_ETH: u64 = 1_000;

#[tokio::test]
#[ignore]
async fn local_node_eth_flow_tx() {
    run_test(eth_flow_tx).await;
}

#[tokio::test]
#[ignore]
async fn local_node_eth_flow_without_quote() {
    run_test(eth_flow_without_quote).await;
}

#[tokio::test]
#[ignore]
async fn local_node_eth_flow_indexing_after_refund() {
    run_test(eth_flow_indexing_after_refund).await;
}

/// Tests that having an order with a buy amount of
/// 0 does not prevent other orders from getting settled.
#[tokio::test]
#[ignore]
async fn local_node_eth_flow_zero_buy_amount() {
    run_test(eth_flow_zero_buy_amount).await;
}

async fn eth_flow_tx(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(2u64.eth()).await;
    let [trader] = onchain.make_accounts(2u64.eth()).await;

    // Create token with Uniswap pool for price estimation
    let [dai] = onchain
        .deploy_tokens_with_weth_uni_v2_pools((DAI_PER_ETH * 1_000).eth(), 1_000u64.eth())
        .await;

    // Get a quote from the services
    let buy_token = *dai.address();
    let receiver = Address::repeat_byte(0x42);
    let sell_amount = 1u64.eth();
    let intent = EthFlowTradeIntent {
        sell_amount,
        buy_token,
        receiver,
    };

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    let approve_call_data = {
        let call_builder = dai.approve(trader.address(), 10u64.eth());
        let calldata = call_builder.calldata();
        const_hex::encode_prefixed(calldata)
    };

    let hash = services
        .put_app_data(
            None,
            &format!(
                r#"{{
    "metadata": {{
         "hooks": {{
             "pre": [
                 {{
                     "target": "{:?}",
                     "callData": "{}",
                     "gasLimit": "100000"
                 }}
             ],
             "post": [
                 {{
                     "target": "{:?}",
                     "callData": "{}",
                     "gasLimit": "100000"
                 }}
             ]
         }}
    }}
}}"#,
                dai.address(),
                approve_call_data,
                onchain.contracts().weth.address(),
                approve_call_data,
            ),
        )
        .await
        .unwrap();

    let quote_request = OrderQuoteRequest {
        app_data: OrderCreationAppData::Hash {
            hash: app_data::AppDataHash(const_hex::decode(&hash[2..]).unwrap().try_into().unwrap()),
        },
        ..intent.to_quote_request(trader.address(), &onchain.contracts().weth)
    };

    let quote: OrderQuoteResponse = test_submit_quote(&services, &quote_request).await;

    let valid_to = chrono::offset::Utc::now().timestamp() as u32
        + timestamp_of_current_block_in_seconds(&web3.alloy)
            .await
            .unwrap()
        + 3600;
    let ethflow_order =
        ExtendedEthFlowOrder::from_quote(&quote, valid_to).include_slippage_bps(300);

    let ethflow_contract = onchain.contracts().ethflows.first().unwrap();
    submit_order(
        &ethflow_order,
        trader.address(),
        onchain.contracts(),
        ethflow_contract,
    )
    .await;

    test_order_availability_in_api(
        &services,
        &ethflow_order,
        &trader.address(),
        onchain.contracts(),
        ethflow_contract,
    )
    .await;

    tracing::info!("waiting for trade");

    test_order_was_settled(&ethflow_order, &onchain).await;

    // make sure the fee was charged for zero fee limit orders
    let fee_charged = || async {
        onchain.mint_block().await;
        let order = services
            .get_order(
                &ethflow_order
                    .uid(onchain.contracts(), ethflow_contract)
                    .await,
            )
            .await
            .unwrap();
        order.metadata.executed_fee > U256::ZERO
    };
    wait_for_condition(TIMEOUT, fee_charged).await.unwrap();

    test_trade_availability_in_api(
        services.client(),
        &ethflow_order,
        &trader.address(),
        onchain.contracts(),
        ethflow_contract,
    )
    .await;

    // Pre and post interactions provided in the appdata got executed.
    // Note that the allowance was set for the trampoline contract
    // which proofs that the interactions were correctly sandboxed.
    let trampoline = *onchain.contracts().hooks.address();
    let allowance = dai
        .allowance(trampoline, trader.address())
        .call()
        .await
        .unwrap();
    assert_eq!(allowance, 10u64.eth());

    let allowance = onchain
        .contracts()
        .weth
        .allowance(trampoline, trader.address())
        .call()
        .await
        .unwrap();
    assert_eq!(allowance, 10u64.eth());

    // Just to be super sure we assert that we indeed were not
    // able to set an allowance on behalf of the settlement contract.
    let settlement = onchain.contracts().gp_settlement.address();
    let allowance = dai
        .allowance(*settlement, trader.address())
        .call()
        .await
        .unwrap();
    assert_eq!(allowance, alloy::primitives::U256::ZERO);

    let allowance = onchain
        .contracts()
        .weth
        .allowance(*settlement, trader.address())
        .call()
        .await
        .unwrap();
    assert_eq!(allowance, alloy::primitives::U256::ZERO);
}

async fn eth_flow_without_quote(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(2u64.eth()).await;
    let [trader] = onchain.make_accounts(2u64.eth()).await;

    // Create token with Uniswap pool for price estimation
    let [dai] = onchain
        .deploy_tokens_with_weth_uni_v2_pools((DAI_PER_ETH * 1_000).eth(), 1_000u64.eth())
        .await;

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    let valid_to = chrono::offset::Utc::now().timestamp() as u32
        + timestamp_of_current_block_in_seconds(&web3.alloy)
            .await
            .unwrap()
        + 3600;
    let ethflow_order = ExtendedEthFlowOrder(CoWSwapEthFlow::EthFlowOrder::Data {
        buyToken: *dai.address(),
        sellAmount: 1u64.eth(),
        buyAmount: alloy::primitives::U256::ONE,
        validTo: valid_to,
        partiallyFillable: false,
        quoteId: 0,
        feeAmount: alloy::primitives::U256::ZERO,
        receiver: Address::from_slice(&[0x42; 20]),
        appData: Default::default(),
    });

    let ethflow_contract = onchain.contracts().ethflows.first().unwrap();
    submit_order(
        &ethflow_order,
        trader.address(),
        onchain.contracts(),
        ethflow_contract,
    )
    .await;

    test_order_availability_in_api(
        &services,
        &ethflow_order,
        &trader.address(),
        onchain.contracts(),
        ethflow_contract,
    )
    .await;

    tracing::info!("waiting for trade");
    test_order_was_settled(&ethflow_order, &onchain).await;
}

async fn eth_flow_indexing_after_refund(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(2u64.eth()).await;
    let [trader, dummy_trader] = onchain.make_accounts(2u64.eth()).await;
    let [dai] = onchain
        .deploy_tokens_with_weth_uni_v2_pools((DAI_PER_ETH * 1000).eth(), 1000u64.eth())
        .await;

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    // Create an order that only exists to be cancelled.
    let valid_to = timestamp_of_current_block_in_seconds(&web3.alloy)
        .await
        .unwrap()
        + 60;
    let dummy_order = ExtendedEthFlowOrder::from_quote(
        &test_submit_quote(
            &services,
            &(EthFlowTradeIntent {
                sell_amount: alloy::primitives::U256::from(42),
                buy_token: *dai.address(),
                receiver: Address::repeat_byte(42),
            })
            .to_quote_request(dummy_trader.address(), &onchain.contracts().weth),
        )
        .await,
        valid_to,
    )
    .include_slippage_bps(300);
    let ethflow_contract = onchain.contracts().ethflows.first().unwrap();
    submit_order(
        &dummy_order,
        dummy_trader.address(),
        onchain.contracts(),
        ethflow_contract,
    )
    .await;
    onchain.mint_block().await;

    dummy_order
        .mine_order_invalidation(dummy_trader.address(), ethflow_contract)
        .await;

    // Create the actual order that should be picked up by the services and matched.
    let buy_token = *dai.address();
    let receiver = Address::repeat_byte(0x42);
    let sell_amount = 1u64.eth();
    let valid_to = chrono::offset::Utc::now().timestamp() as u32
        + timestamp_of_current_block_in_seconds(&web3.alloy)
            .await
            .unwrap()
        + 60;
    let ethflow_order = ExtendedEthFlowOrder::from_quote(
        &test_submit_quote(
            &services,
            &(EthFlowTradeIntent {
                sell_amount,
                buy_token,
                receiver,
            })
            .to_quote_request(trader.address(), &onchain.contracts().weth),
        )
        .await,
        valid_to,
    )
    .include_slippage_bps(300);
    submit_order(
        &ethflow_order,
        trader.address(),
        onchain.contracts(),
        ethflow_contract,
    )
    .await;

    tracing::info!("waiting for trade");
    test_order_was_settled(&ethflow_order, &onchain).await;

    // Check order events
    let events = crate::database::events_of_order(
        services.db(),
        &dummy_order.uid(onchain.contracts(), ethflow_contract).await,
    )
    .await;
    assert_eq!(events.first().unwrap().label, OrderEventLabel::Created);
    assert_eq!(events.last().unwrap().label, OrderEventLabel::Cancelled);
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
    let approx_output: U256 = response.quote.sell_amount * U256::from(DAI_PER_ETH);
    assert!(response.quote.buy_amount > (approx_output * U256::from(9u64) / U256::from(10)));
    assert!(response.quote.buy_amount < (approx_output * U256::from(11u64) / U256::from(10)));

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

async fn submit_order(
    ethflow_order: &ExtendedEthFlowOrder,
    user: Address,
    contracts: &Contracts,
    ethflow_contract: &CoWSwapEthFlow::Instance,
) {
    assert_eq!(
        ethflow_order.status(contracts, ethflow_contract).await,
        RefundStatus::Invalid
    );

    let result = ethflow_order
        .mine_order_creation(user, ethflow_contract)
        .await;
    assert!(result.status()); // success
    assert_eq!(
        ethflow_order.status(contracts, ethflow_contract).await,
        RefundStatus::NotYetRefunded(user)
    );
}

async fn test_order_availability_in_api(
    services: &Services<'_>,
    order: &ExtendedEthFlowOrder,
    owner: &Address,
    contracts: &Contracts,
    ethflow_contract: &CoWSwapEthFlow::Instance,
) {
    tracing::info!("Waiting for order to show up in API.");
    let uid = order.uid(contracts, ethflow_contract).await;
    let is_available = || async { services.get_order(&uid).await.is_ok() };
    wait_for_condition(TIMEOUT, is_available).await.unwrap();

    test_orders_query(services, order, owner, contracts, ethflow_contract).await;

    // Api returns eth flow orders for both eth-flow contract address and actual
    // owner
    for address in [owner, ethflow_contract.address()] {
        test_account_query(
            address,
            services.client(),
            order,
            owner,
            contracts,
            ethflow_contract,
        )
        .await;
    }
}

async fn test_trade_availability_in_api(
    client: &Client,
    order: &ExtendedEthFlowOrder,
    owner: &Address,
    contracts: &Contracts,
    ethflow_contract: &CoWSwapEthFlow::Instance,
) {
    test_trade_query(
        &TradeQuery::ByUid(order.uid(contracts, ethflow_contract).await),
        client,
        contracts,
        ethflow_contract,
    )
    .await;

    // Api returns eth flow orders for both eth-flow contract address and actual
    // owner
    for address in [owner, ethflow_contract.address()] {
        test_trade_query(
            &TradeQuery::ByOwner(*address),
            client,
            contracts,
            ethflow_contract,
        )
        .await;
    }
}

async fn test_order_was_settled(ethflow_order: &ExtendedEthFlowOrder, onchain: &OnchainComponents) {
    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;
        let buy_token =
            ERC20Mintable::Instance::new(ethflow_order.0.buyToken, onchain.web3().alloy.clone());
        let receiver_buy_token_balance = buy_token
            .balanceOf(ethflow_order.0.receiver)
            .call()
            .await
            .expect("Unable to get token balance");

        receiver_buy_token_balance >= ethflow_order.0.buyAmount
    })
    .await
    .unwrap();
}

async fn test_orders_query(
    services: &Services<'_>,
    order: &ExtendedEthFlowOrder,
    owner: &Address,
    contracts: &Contracts,
    ethflow_contract: &CoWSwapEthFlow::Instance,
) {
    let response = services
        .get_order(&order.uid(contracts, ethflow_contract).await)
        .await
        .unwrap();
    test_order_parameters(&response, order, owner, contracts, ethflow_contract).await;
}

async fn test_account_query(
    queried_account: &Address,
    client: &Client,
    order: &ExtendedEthFlowOrder,
    owner: &Address,
    contracts: &Contracts,
    ethflow_contract: &CoWSwapEthFlow::Instance,
) {
    let query = client
        .get(format!(
            "{API_HOST}{ACCOUNT_ENDPOINT}/{queried_account:?}/orders",
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(query.status(), 200);
    let response = query.json::<Vec<Order>>().await.unwrap();
    let uid = order.uid(contracts, ethflow_contract).await;
    let target_order = response.iter().find(|o| o.metadata.uid == uid).unwrap();
    test_order_parameters(target_order, order, owner, contracts, ethflow_contract).await;
}

enum TradeQuery {
    ByUid(OrderUid),
    ByOwner(Address),
}

async fn test_trade_query(
    query_type: &TradeQuery,
    client: &Client,
    contracts: &Contracts,
    ethflow_contract: &CoWSwapEthFlow::Instance,
) {
    let query = client
        .get(format!("{API_HOST}{TRADES_ENDPOINT}",))
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
    assert_eq!(response[0].owner, *ethflow_contract.address());
    assert_eq!(response[0].sell_token, *contracts.weth.address());
}

async fn test_order_parameters(
    response: &Order,
    order: &ExtendedEthFlowOrder,
    owner: &Address,
    contracts: &Contracts,
    ethflow_contract: &CoWSwapEthFlow::Instance,
) {
    // Expected values from actual EIP1271 order instead of eth-flow order
    assert_eq!(response.data.valid_to, u32::MAX);
    assert_eq!(response.metadata.owner, *ethflow_contract.address());
    assert_eq!(response.data.sell_token, *contracts.weth.address());

    // Specific parameters return the missing values
    assert_eq!(
        response.metadata.ethflow_data,
        Some(EthflowData {
            user_valid_to: order.0.validTo as i64,
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
    assert_eq!(response.metadata.class, OrderClass::Limit);
    assert!(
        order
            .is_valid_cowswap_signature(&response.signature, contracts, ethflow_contract)
            .await
            .is_ok()
    );

    // Requires wrapping first
    assert!(!response.interactions.pre.is_empty());
    assert_eq!(
        response.interactions.pre[0].target,
        *ethflow_contract.address()
    );
    assert_eq!(response.interactions.pre[0].call_data, WRAP_ALL_SELECTOR);
}

pub struct ExtendedEthFlowOrder(pub CoWSwapEthFlow::EthFlowOrder::Data);

impl ExtendedEthFlowOrder {
    pub fn from_quote(quote_response: &OrderQuoteResponse, valid_to: u32) -> Self {
        let quote = &quote_response.quote;
        ExtendedEthFlowOrder(CoWSwapEthFlow::EthFlowOrder::Data {
            buyToken: quote.buy_token,
            receiver: quote.receiver.expect("eth-flow order without receiver"),
            sellAmount: quote.sell_amount,
            buyAmount: quote.buy_amount,
            appData: quote.app_data.hash().0.into(),
            feeAmount: alloy::primitives::U256::ZERO,
            validTo: valid_to, // note: valid to in the quote is always unlimited
            partiallyFillable: quote.partially_fillable,
            quoteId: quote_response.id.expect("No quote id"),
        })
    }

    fn to_cow_swap_order(
        &self,
        ethflow_contract: &CoWSwapEthFlow::Instance,
        weth: &WETH9::Instance,
    ) -> Order {
        // Each ethflow user order has an order that is representing
        // it as EIP1271 order with a different owner and valid_to
        OrderBuilder::default()
            .with_kind(OrderKind::Sell)
            .with_sell_token(*weth.address())
            .with_sell_amount(self.0.sellAmount)
            .with_fee_amount(self.0.feeAmount)
            .with_receiver(Some(self.0.receiver))
            .with_buy_token(self.0.buyToken)
            .with_buy_amount(self.0.buyAmount)
            .with_valid_to(u32::MAX)
            .with_app_data(self.0.appData.0)
            .with_class(OrderClass::Market) // Eth-flow orders only support market orders at this point in time
            .with_eip1271(*ethflow_contract.address(), vec![])
            .build()
    }

    pub fn include_slippage_bps(&self, slippage: u16) -> Self {
        const MAX_BASE_POINT: u16 = 10000;
        if slippage > MAX_BASE_POINT {
            panic!("Slippage must be specified in base points");
        }
        ExtendedEthFlowOrder(CoWSwapEthFlow::EthFlowOrder::Data {
            buyAmount: self.0.buyAmount * alloy::primitives::U256::from(MAX_BASE_POINT - slippage)
                / alloy::primitives::U256::from(MAX_BASE_POINT),
            ..self.0
        })
    }

    pub async fn status(
        &self,
        contracts: &Contracts,
        ethflow_contract: &CoWSwapEthFlow::Instance,
    ) -> RefundStatus {
        ethflow_contract
            .orders(self.hash(contracts, ethflow_contract).await)
            .call()
            .await
            .expect("Couldn't fetch order status")
            .into()
    }

    pub async fn is_valid_cowswap_signature(
        &self,
        cowswap_signature: &Signature,
        contracts: &Contracts,
        ethflow_contract: &CoWSwapEthFlow::Instance,
    ) -> anyhow::Result<()> {
        let bytes = match cowswap_signature {
            Signature::Eip1271(bytes) => bytes,
            _ => bail!(
                "Invalid signature type, expected EIP1271, found {:?}",
                cowswap_signature
            ),
        }
        .clone();

        let result = ethflow_contract
            .isValidSignature(
                self.hash(contracts, ethflow_contract).await,
                Bytes::from(bytes),
            )
            .call()
            .await
            .expect("Couldn't fetch signature validity");

        check_erc1271_result(result)
            .map_err(|err| anyhow::anyhow!("failed signature verification: {:?}", err))
    }

    pub async fn mine_order_creation(
        &self,
        owner: Address,
        ethflow_contract: &CoWSwapEthFlow::Instance,
    ) -> TransactionReceipt {
        ethflow_contract
            .createOrder(self.0.clone())
            .value(self.0.sellAmount + self.0.feeAmount)
            .from(owner)
            .send()
            .await
            .unwrap()
            .get_receipt()
            .await
            .unwrap()
    }

    pub async fn mine_order_invalidation(
        &self,
        sender: Address,
        ethflow_contract: &CoWSwapEthFlow::Instance,
    ) {
        ethflow_contract
            .invalidateOrder(self.0.clone())
            .from(sender)
            .send_and_watch()
            .await
            .unwrap();
    }

    async fn hash(
        &self,
        contracts: &Contracts,
        ethflow_contract: &CoWSwapEthFlow::Instance,
    ) -> B256 {
        let domain_separator = DomainSeparator(
            contracts
                .gp_settlement
                .domainSeparator()
                .call()
                .await
                .expect("Couldn't query domain separator")
                .0,
        );
        hashed_eip712_message(
            &domain_separator,
            &self
                .to_cow_swap_order(ethflow_contract, &contracts.weth)
                .data
                .hash_struct(),
        )
    }

    pub async fn uid(
        &self,
        contracts: &Contracts,
        ethflow_contract: &CoWSwapEthFlow::Instance,
    ) -> OrderUid {
        let domain_separator = DomainSeparator(
            contracts
                .gp_settlement
                .domainSeparator()
                .call()
                .await
                .expect("Couldn't query domain separator")
                .0,
        );
        self.to_cow_swap_order(ethflow_contract, &contracts.weth)
            .data
            .uid(&domain_separator, *ethflow_contract.address())
    }
}

pub struct EthFlowTradeIntent {
    pub sell_amount: alloy::primitives::U256,
    pub buy_token: Address,
    pub receiver: Address,
}

impl EthFlowTradeIntent {
    // How a user trade intent is converted into a quote request by the frontend
    pub fn to_quote_request(&self, from: Address, weth: &WETH9::Instance) -> OrderQuoteRequest {
        OrderQuoteRequest {
            from,
            // Even if the user sells ETH, we request a quote for WETH
            sell_token: *weth.address(),
            buy_token: self.buy_token,
            receiver: Some(self.receiver),
            validity: Validity::For(3600),
            app_data: OrderCreationAppData::default(),
            signing_scheme: QuoteSigningScheme::Eip1271 {
                onchain_order: true,
                verification_gas_limit: 0,
            },
            side: OrderQuoteSide::Sell {
                sell_amount: model::quote::SellAmount::AfterFee {
                    value: NonZeroU256::try_from(self.sell_amount).unwrap(),
                },
            },
            buy_token_balance: BuyTokenDestination::Erc20,
            sell_token_balance: SellTokenSource::Erc20,
            price_quality: PriceQuality::Optimal,
            timeout: Default::default(),
        }
    }
}

async fn eth_flow_zero_buy_amount(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(2u64.eth()).await;
    let [trader_a, trader_b] = onchain.make_accounts(2u64.eth()).await;

    // Create token with Uniswap pool for price estimation
    let [dai] = onchain
        .deploy_tokens_with_weth_uni_v2_pools((DAI_PER_ETH * 1_000).eth(), 1_000u64.eth())
        .await;

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    let place_order = async |trader: TestAccount, buy_amount: u64| {
        let valid_to = chrono::offset::Utc::now().timestamp() as u32
            + timestamp_of_current_block_in_seconds(&web3.alloy)
                .await
                .unwrap()
            + 3600;
        let ethflow_order = ExtendedEthFlowOrder(CoWSwapEthFlow::EthFlowOrder::Data {
            buyToken: *dai.address(),
            sellAmount: 1u64.eth(),
            buyAmount: alloy::primitives::U256::from(buy_amount),
            validTo: valid_to,
            partiallyFillable: false,
            quoteId: 0,
            feeAmount: alloy::primitives::U256::ZERO,
            receiver: Address::from_slice(&[0x42; 20]),
            appData: Default::default(),
        });

        let ethflow_contract = onchain.contracts().ethflows.first().unwrap();
        submit_order(
            &ethflow_order,
            trader.address(),
            onchain.contracts(),
            ethflow_contract,
        )
        .await;

        test_order_availability_in_api(
            &services,
            &ethflow_order,
            &trader.address(),
            onchain.contracts(),
            ethflow_contract,
        )
        .await;
        ethflow_order
    };

    // In the past this would have been an order that caused the
    // whole auction to be discarded. We place it first to ensure
    // it's part of the auction to prevent our "good" order getting
    // settled before we can place the "bad" order.
    let _ = place_order(trader_a, 0).await;
    let order_b = place_order(trader_b, 1).await;

    // Although the auction contains a problematic order we can
    // still settle good orders.
    tracing::info!("waiting for trade");
    test_order_was_settled(&order_b, &onchain).await;
}
