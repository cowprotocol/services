use {
    contracts::ERC20,
    e2e::{
        nodes::forked_node::ForkedNodeApi,
        setup::{
            run_forked_test_with_block_number,
            to_wei,
            to_wei_with_exp,
            OnchainComponents,
            Services,
        },
        tx,
    },
    ethcontract::{prelude::U256, H160},
    ethrpc::Web3,
    model::quote::{OrderQuoteRequest, OrderQuoteSide, SellAmount},
    reqwest::StatusCode,
};

#[tokio::test]
#[ignore]
async fn f0rked_node_mainnet_single_limit_order() {
    run_forked_test_with_block_number(
        forked_mainnet_onchain_banned_user_test,
        std::env::var("FORK_URL_MAINNET")
            .expect("FORK_URL_MAINNET must be set to run forked tests"),
        FORK_BLOCK_MAINNET,
    )
    .await;
}

/// The block number from which we will fetch state for the forked tests.
const FORK_BLOCK_MAINNET: u64 = 18477910;
/// DAI whale address as per [FORK_BLOCK_MAINNET].
const DAI_WHALE_MAINNET: H160 = H160(hex_literal::hex!(
    "075e72a5eDf65F0A5f44699c7654C1a76941Ddc8"
));
const BANNED_USER: H160 = H160(hex_literal::hex!(
    "7F367cC41522cE07553e823bf3be79A889DEbe1B"
));

async fn forked_mainnet_onchain_banned_user_test(web3: Web3) {
    let mut onchain = OnchainComponents::deployed(web3.clone()).await;
    let [solver] = onchain.make_solvers_forked(to_wei(1)).await;
    let forked_node_api = web3.api::<ForkedNodeApi<_>>();

    let token_dai = ERC20::at(
        &web3,
        "0x6b175474e89094c44da98b954eedeac495271d0f"
            .parse()
            .unwrap(),
    );

    let token_usdt = ERC20::at(
        &web3,
        "0xdac17f958d2ee523a2206206994597c13d831ec7"
            .parse()
            .unwrap(),
    );

    let banned_user = forked_node_api.impersonate(&BANNED_USER).await.unwrap();

    // Give trader some DAI
    let dai_whale = forked_node_api
        .impersonate(&DAI_WHALE_MAINNET)
        .await
        .unwrap();
    tx!(
        dai_whale,
        token_dai.transfer(banned_user.address(), to_wei_with_exp(1000, 18))
    );

    // Approve GPv2 for trading
    tx!(
        banned_user,
        token_dai.approve(onchain.contracts().allowance, to_wei_with_exp(1000, 18))
    );

    // Place Order
    let services = Services::new(onchain.contracts()).await;
    services.start_protocol(solver).await;

    let result = services
        .submit_quote(&OrderQuoteRequest {
            sell_token: token_dai.address(),
            buy_token: token_usdt.address(),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: to_wei_with_exp(1000, 18).try_into().unwrap(),
                },
            },
            from: banned_user.address(),
            ..Default::default()
        })
        .await;
    assert!(matches!(result, Err((StatusCode::FORBIDDEN, _))));
}
