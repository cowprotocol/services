use {
    alloy::{
        primitives::{Address, address},
        providers::ext::{AnvilApi, ImpersonateConfig},
    },
    contracts::alloy::ERC20,
    e2e::setup::{
        OnchainComponents,
        Services,
        eth,
        run_forked_test_with_block_number,
        to_wei,
        to_wei_with_exp,
    },
    ethrpc::{
        Web3,
        alloy::conversions::{IntoAlloy, IntoLegacy},
    },
    model::quote::{OrderQuoteRequest, OrderQuoteSide, SellAmount},
    reqwest::StatusCode,
};

#[tokio::test]
#[ignore]
async fn forked_node_mainnet_single_limit_order() {
    run_forked_test_with_block_number(
        forked_mainnet_onchain_banned_user_test,
        std::env::var("FORK_URL_MAINNET")
            .expect("FORK_URL_MAINNET must be set to run forked tests"),
        FORK_BLOCK_MAINNET,
    )
    .await;
}

/// The block number from which we will fetch state for the forked tests.
const FORK_BLOCK_MAINNET: u64 = 23112197;
/// DAI whale address as per [FORK_BLOCK_MAINNET].
const DAI_WHALE_MAINNET: Address = address!("762d46904B93a1EEDBfF2fD50445CB8ffA41F9FB");
const BANNED_USER: Address = address!("7F367cC41522cE07553e823bf3be79A889DEbe1B");

async fn forked_mainnet_onchain_banned_user_test(web3: Web3) {
    let mut onchain = OnchainComponents::deployed(web3.clone()).await;
    let [solver] = onchain.make_solvers_forked(to_wei(1)).await;

    let token_dai = ERC20::Instance::new(
        address!("6b175474e89094c44da98b954eedeac495271d0f"),
        web3.alloy.clone(),
    );

    let token_usdt = ERC20::Instance::new(
        address!("dac17f958d2ee523a2206206994597c13d831ec7"),
        web3.alloy.clone(),
    );

    web3.alloy
        .anvil_send_impersonated_transaction_with_config(
            token_dai
                .transfer(BANNED_USER, to_wei_with_exp(1000, 18).into_alloy())
                .from(DAI_WHALE_MAINNET)
                .into_transaction_request(),
            ImpersonateConfig {
                fund_amount: None,
                stop_impersonate: true,
            },
        )
        .await
        .unwrap()
        .get_receipt()
        .await
        .unwrap();

    // Approve GPv2 for trading
    web3.alloy
        .anvil_send_impersonated_transaction_with_config(
            token_dai
                .approve(
                    onchain.contracts().allowance.into_alloy(),
                    to_wei_with_exp(1000, 18).into_alloy(),
                )
                .from(BANNED_USER)
                .into_transaction_request(),
            ImpersonateConfig {
                fund_amount: Some(eth(1)),
                stop_impersonate: true,
            },
        )
        .await
        .unwrap()
        .get_receipt()
        .await
        .unwrap();

    // Place Order
    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    let result = services
        .submit_quote(&OrderQuoteRequest {
            sell_token: token_dai.address().into_legacy(),
            buy_token: token_usdt.address().into_legacy(),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: to_wei_with_exp(1000, 18).try_into().unwrap(),
                },
            },
            from: BANNED_USER.into_legacy(),
            ..Default::default()
        })
        .await;
    assert!(matches!(result, Err((StatusCode::FORBIDDEN, _))));
}
