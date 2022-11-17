use crate::{
    eth_flow::{EthFlowOrderOnchainStatus, ExtendedEthFlowOrder},
    local_node::AccountAssigner,
    services::{
        deploy_token_with_weth_uniswap_pool, to_wei, MintableToken, OrderbookServices,
        WethPoolConfig, API_HOST,
    },
};
use ethcontract::{H160, U256};
use model::quote::{
    OrderQuoteRequest, OrderQuoteResponse, OrderQuoteSide, QuoteSigningScheme, Validity,
};
use refunder::refund_service::RefundService;
use shared::{ethrpc::Web3, http_client::HttpClientFactory, maintenance::Maintaining};
use sqlx::PgPool;

const QUOTING_ENDPOINT: &str = "/api/v1/quote/";

#[tokio::test]
#[ignore]
async fn local_node_refunder_tx() {
    crate::local_node::test(refunder_tx).await;
}

async fn refunder_tx(web3: Web3) {
    shared::tracing::initialize_for_tests("warn,orderbook=debug,solver=debug,autopilot=debug");
    shared::exit_process_on_panic::set_panic_hook();
    let contracts = crate::deploy::deploy(&web3).await.expect("deploy");

    let mut accounts = AccountAssigner::new(&web3).await;
    let user = accounts.assign_free_account();
    let refunder_account = accounts.assign_free_account();

    // Create token with Uniswap pool for price estimation
    let MintableToken {
        contract: token, ..
    } = deploy_token_with_weth_uniswap_pool(
        &web3,
        &contracts,
        WethPoolConfig {
            token_amount: to_wei(100_000),
            weth_amount: to_wei(100_000),
        },
    )
    .await;

    let services = OrderbookServices::new(&web3, &contracts, true).await;
    let http_factory = HttpClientFactory::default();
    let client = http_factory.create();

    // Get quote id for order placement
    let buy_token = token.address();
    let receiver = Some(H160([42; 20]));
    let sell_amount = U256::from("3000000000000000");

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

    // A valid_to in the past is chosen, such that the refunder can refund it immediately
    let valid_to = chrono::offset::Utc::now().timestamp() as u32 - 1;
    // Accounting for slippage is necesary for the order to be picked up by the refunder
    let ethflow_order =
        ExtendedEthFlowOrder::from_quote(&quote_response, valid_to).include_slippage_bps(9999);

    ethflow_order
        .mine_order_creation(&user, &contracts.ethflow)
        .await;

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

    assert_ne!(
        ethflow_order.status(&contracts).await,
        EthFlowOrderOnchainStatus::Invalidated
    );

    refunder.try_to_refund_all_eligble_orders().await.unwrap();

    assert_eq!(
        ethflow_order.status(&contracts).await,
        EthFlowOrderOnchainStatus::Invalidated
    );
}
