use crate::services::{deploy_mintable_token, to_wei, OrderbookServices, API_HOST};
use ethcontract::{Account, Address, Bytes, H256, U256};
use model::{
    order::{OrderBuilder, OrderKind},
    quote::{OrderQuoteRequest, OrderQuoteResponse, OrderQuoteSide, QuoteSigningScheme, Validity},
    signature::hashed_eip712_message,
    DomainSeparator,
};
use refunder::refund_service::{EncodedEthflowOrder, RefundService, INVALIDATED_OWNER};
use shared::{ethrpc::Web3, http_client::HttpClientFactory, maintenance::Maintaining};
use sqlx::PgPool;

const QUOTING_ENDPOINT: &str = "/api/v1/quote/";

#[tokio::test]
#[ignore]
async fn local_node_smart_contract_orders() {
    crate::local_node::test(refunder_tx).await;
}

async fn refunder_tx(web3: Web3) {
    shared::tracing::initialize_for_tests("warn,orderbook=debug,solver=debug,autopilot=debug");
    shared::exit_process_on_panic::set_panic_hook();
    let contracts = crate::deploy::deploy(&web3).await.expect("deploy");

    let accounts: Vec<Address> = web3.eth().accounts().await.expect("get accounts failed");
    let setup_account = Account::Local(accounts[0], None);
    let user = Account::Local(accounts[1], None);
    let refunder_account = Account::Local(accounts[2], None);

    // Create & Mint tokens to trade
    let token = deploy_mintable_token(&web3).await;
    tx!(
        setup_account,
        token.mint(setup_account.address(), to_wei(100_000))
    );
    tx_value!(setup_account, to_wei(100_000), contracts.weth.deposit());

    // Create and fund Uniswap pool for price estimation
    tx!(
        setup_account,
        contracts
            .uniswap_factory
            .create_pair(token.address(), contracts.weth.address())
    );
    tx!(
        setup_account,
        token.approve(contracts.uniswap_router.address(), to_wei(100_000))
    );
    tx!(
        setup_account,
        contracts
            .weth
            .approve(contracts.uniswap_router.address(), to_wei(100_000))
    );
    tx!(
        setup_account,
        contracts.uniswap_router.add_liquidity(
            token.address(),
            contracts.weth.address(),
            to_wei(100_000),
            to_wei(100_000),
            0_u64.into(),
            0_u64.into(),
            setup_account.address(),
            U256::max_value(),
        )
    );

    let services = OrderbookServices::new(&web3, &contracts, true).await;
    let http_factory = HttpClientFactory::default();
    let client = http_factory.create();

    // Get quote id for order placement
    let buy_token = token.address();
    let receiver = Some(setup_account.address());
    let sell_amount = U256::from("3000000000000000");
    let buy_amount = U256::from("1");
    // A valid_to in the past is chosen, such that the refunder can refund it immediately
    let valid_to = chrono::offset::Utc::now().timestamp() as u32 - 1;
    let quote = OrderQuoteRequest {
        from: contracts.ethflow.address(),
        sell_token: contracts.weth.address(),
        buy_token,
        receiver,
        validity: Validity::For(u32::MAX),
        signing_scheme: QuoteSigningScheme::Eip1271 {
            onchain_order: true,
            verification_gas_limit: 0,
        },
        side: OrderQuoteSide::Sell {
            sell_amount: model::quote::SellAmount::AfterFee { value: sell_amount },
        },
        ..Default::default()
    };
    let quoting = client
        .post(&format!("{API_HOST}{QUOTING_ENDPOINT}"))
        .json(&quote)
        .send()
        .await
        .unwrap();
    assert_eq!(quoting.status(), 200);
    let quote_response = quoting.json::<OrderQuoteResponse>().await.unwrap();
    let quote_id: i64 = quote_response.id.unwrap();
    let fee_amount = quote_response.quote.fee_amount;

    // Creating ethflow order
    let ethflow_order: EncodedEthflowOrder = (
        buy_token,
        receiver.unwrap(),
        sell_amount,
        buy_amount,
        Bytes([0u8; 32]),
        fee_amount,
        valid_to,
        false,
        quote_id,
    );

    // Each ethflow user order has an order that is representing
    // it as EIP1271 order with a different owner and valid_to
    let technical_order = OrderBuilder::default()
        .with_kind(OrderKind::Sell)
        .with_sell_token(contracts.weth.address())
        .with_sell_amount(sell_amount)
        .with_fee_amount(fee_amount)
        .with_receiver(receiver)
        .with_buy_token(token.address())
        .with_buy_amount(buy_amount)
        .with_valid_to(u32::MAX)
        .with_eip1271(
            contracts.ethflow.address(),
            contracts.ethflow.address().0.to_vec(),
        )
        .build();
    let domain_separator = DomainSeparator(
        contracts
            .gp_settlement
            .domain_separator()
            .call()
            .await
            .expect("Couldn't query domain separator")
            .0,
    );
    let order_hash = H256(hashed_eip712_message(
        &domain_separator,
        &technical_order.data.hash_struct(),
    ));

    // Mine ethflow order
    tx_value!(
        user,
        sell_amount + fee_amount,
        contracts.ethflow.create_order(ethflow_order)
    );

    // Run autopilot indexing loop
    services.maintenance.run_maintenance().await.unwrap();

    // Create the refund service and execute the refund tx
    let pg_pool = PgPool::connect_lazy("postgresql://").expect("failed to create database");
    let mut refunder = RefundService::new(
        pg_pool,
        web3,
        contracts.ethflow.clone(),
        -3i64, // Needs to be negative, as valid to was chosen to be in the past
        10u64,
        refunder_account,
    );

    let order_status = contracts
        .ethflow
        .orders(Bytes(order_hash.0))
        .call()
        .await
        .expect("Couldn't fetch native token balance");
    assert_ne!(order_status.0, INVALIDATED_OWNER);

    refunder.try_to_refund_all_eligble_orders().await.unwrap();

    // Observe that the order got invalidated
    let order_status = contracts
        .ethflow
        .orders(Bytes(order_hash.0))
        .call()
        .await
        .expect("Couldn't fetch native token balance");
    assert_eq!(order_status.0, INVALIDATED_OWNER);
}
