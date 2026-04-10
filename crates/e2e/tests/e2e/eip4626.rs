use {
    ::alloy::{
        primitives::{Address, U256, address},
        providers::ext::{AnvilApi, ImpersonateConfig},
    },
    configs::{
        autopilot::{Configuration, native_price::NativePriceConfig},
        native_price_estimators::{NativePriceEstimator, NativePriceEstimators},
        test_util::TestDefault,
    },
    contracts::ERC20,
    e2e::setup::*,
    ethrpc::alloy::CallBuilderExt,
    model::quote::{OrderQuoteRequest, OrderQuoteSide, SellAmount},
    number::units::EthUnit,
    shared::web3::Web3,
};

/// The block number from which we will fetch state for the forked test.
const FORK_BLOCK_MAINNET: u64 = 23112197;

/// sDAI (Savings DAI) – an EIP-4626 vault wrapping DAI.
const SDAI: Address = address!("83F20F44975D03b1b09e64809B757c47f942BEeA");

/// sDAI whale at [`FORK_BLOCK_MAINNET`].
const SDAI_WHALE: Address = address!("4C612E3B15b96Ff9A6faED838F8d07d479a8dD4c");

/// WETH on mainnet.
const WETH: Address = address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");

#[tokio::test]
#[ignore]
async fn forked_node_mainnet_eip4626_native_price() {
    run_forked_test_with_block_number(
        eip4626_native_price_test,
        std::env::var("FORK_URL_MAINNET")
            .expect("FORK_URL_MAINNET must be set to run forked tests"),
        FORK_BLOCK_MAINNET,
    )
    .await;
}

async fn eip4626_native_price_test(web3: Web3) {
    let mut onchain = OnchainComponents::deployed(web3.clone()).await;

    let [solver] = onchain.make_solvers_forked(1u64.eth()).await;
    let [trader] = onchain.make_accounts(1u64.eth()).await;

    let sdai = ERC20::Instance::new(SDAI, web3.provider.clone());

    // Transfer sDAI from whale to trader.
    web3.provider
        .anvil_send_impersonated_transaction_with_config(
            sdai.transfer(trader.address(), 1000u64.eth())
                .from(SDAI_WHALE)
                .into_transaction_request(),
            ImpersonateConfig {
                fund_amount: Some(1u64.eth()),
                stop_impersonate: true,
            },
        )
        .await
        .unwrap()
        .get_receipt()
        .await
        .unwrap();

    // Approve the vault-relayer for trading.
    sdai.approve(onchain.contracts().allowance, 1000u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    // Stage 1: EIP-4626 chain — vault tokens priced via conversion rate.
    // Stage 2: driver fallback for non-vault tokens. The autopilot prices
    //          WETH at startup and panics if it can't, so a plain driver
    //          stage is required even though we're only testing vaults.
    // results_required=1 so stage 2 only runs when stage 1 fails.
    let driver_url: url::Url = "http://localhost:11088/test_solver".parse().unwrap();
    let autopilot_config = Configuration {
        native_price_estimation: NativePriceConfig {
            estimators: NativePriceEstimators::new(vec![
                vec![
                    NativePriceEstimator::eip4626(1.try_into().unwrap()),
                    NativePriceEstimator::driver("test_quoter".to_string(), driver_url.clone()),
                ],
                vec![NativePriceEstimator::driver(
                    "test_quoter".to_string(),
                    driver_url,
                )],
            ]),
            shared: configs::native_price::NativePriceConfig {
                results_required: 1.try_into().unwrap(),
                ..Default::default()
            },
            ..NativePriceConfig::test_default()
        },
        ..Configuration::test("test_solver", solver.address())
    };

    let services = Services::new(&onchain).await;
    services
        .start_protocol_with_args(
            autopilot_config,
            configs::orderbook::Configuration::test_default(),
            solver,
        )
        .await;

    onchain.mint_block().await;

    // Submit a quote selling sDAI for WETH. If the EIP-4626 native price
    // estimator works, the protocol can price sDAI and the quote succeeds.
    let quote = services
        .submit_quote(&OrderQuoteRequest {
            from: trader.address(),
            sell_token: SDAI,
            buy_token: WETH,
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: (100u64.eth()).try_into().unwrap(),
                },
            },
            ..Default::default()
        })
        .await;

    assert!(
        quote.is_ok(),
        "quote for sDAI should succeed with EIP-4626 native price estimator: {:?}",
        quote.err()
    );
}

#[tokio::test]
#[ignore]
async fn forked_node_mainnet_eip4626_recursive_native_price() {
    run_forked_test_with_block_number(
        eip4626_recursive_native_price_test,
        std::env::var("FORK_URL_MAINNET")
            .expect("FORK_URL_MAINNET must be set to run forked tests"),
        FORK_BLOCK_MAINNET,
    )
    .await;
}

/// Tests pricing and quoting of recursive EIP-4626 vaults with non-trivial
/// conversion rates. Deploys mock wrapper vaults on top of sDAI (which itself
/// wraps DAI) with different rates, seeds Uniswap V2 pools so the solver can
/// find routes, and verifies both native prices and full quotes.
async fn eip4626_recursive_native_price_test(web3: Web3) {
    let mut onchain = OnchainComponents::deployed(web3.clone()).await;

    let [solver] = onchain.make_solvers_forked(1u64.eth()).await;
    let [trader] = onchain.make_accounts(100u64.eth()).await;

    // Deploy mock EIP-4626 vaults wrapping sDAI with different conversion rates.
    // Each wrapper applies `convertToAssets(shares) = shares * num / den`, so a
    // (3, 2) wrapper means 1 share = 1.5 sDAI, making it 1.5x the sDAI price.
    let rates: &[(u64, u64)] = &[(3, 2), (2, 1), (1, 3)];
    let mut wrappers = Vec::with_capacity(rates.len());
    for &(num, den) in rates {
        let wrapper = contracts::test::MockERC4626Wrapper::Instance::deploy(
            web3.provider.clone(),
            SDAI,
            18u8,
            U256::from(num),
            U256::from(den),
        )
        .await
        .unwrap();
        let mintable =
            MintableToken::at(*wrapper.address(), trader.address(), web3.provider.clone());
        wrappers.push(mintable);
    }

    // Seed Uniswap V2 pools so the solver can find routes for the wrapper
    // tokens. We pair each wrapper with WETH.
    let weth_token = MintableToken::at(
        *onchain.contracts().weth.address(),
        trader.address(),
        web3.provider.clone(),
    );
    onchain
        .contracts()
        .weth
        .deposit()
        .value(U256::from(rates.len() as u64) * 10u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();
    for wrapper in &wrappers {
        onchain
            .seed_uni_v2_pool((wrapper, 10_000u64.eth()), (&weth_token, 10u64.eth()))
            .await;
    }

    // Stage 1: EIP-4626 chain — vault tokens priced via conversion rate.
    // Stage 2: driver fallback for non-vault tokens. The autopilot prices
    //          WETH at startup and panics if it can't, so a plain driver
    //          stage is required even though we're only testing vaults.
    // results_required=1 so stage 2 only runs when stage 1 fails.
    let driver_url: url::Url = "http://localhost:11088/test_solver".parse().unwrap();
    let autopilot_config = Configuration {
        native_price_estimation: NativePriceConfig {
            estimators: NativePriceEstimators::new(vec![
                vec![
                    NativePriceEstimator::eip4626(2.try_into().unwrap()),
                    NativePriceEstimator::driver("test_quoter".to_string(), driver_url.clone()),
                ],
                vec![NativePriceEstimator::driver(
                    "test_quoter".to_string(),
                    driver_url,
                )],
            ]),
            shared: configs::native_price::NativePriceConfig {
                results_required: 1.try_into().unwrap(),
                ..Default::default()
            },
            ..NativePriceConfig::test_default()
        },
        ..Configuration::test("test_solver", solver.address())
    };

    let services = Services::new(&onchain).await;
    services
        .start_protocol_with_args(
            autopilot_config,
            configs::orderbook::Configuration::test_default(),
            solver,
        )
        .await;

    onchain.mint_block().await;

    // Verify native prices: the ratio between any two wrapper prices should
    // match the ratio of their vault conversion rates.
    let mut prices = Vec::with_capacity(rates.len());
    for (wrapper, &(num, den)) in wrappers.iter().zip(rates) {
        let addr = *wrapper.address();
        wait_for_condition(TIMEOUT, || async {
            services.get_native_price(&addr).await.is_ok()
        })
        .await
        .unwrap_or_else(|_| panic!("native price for wrapper ({num}/{den}) should be available"));

        prices.push(services.get_native_price(&addr).await.unwrap().price);
    }

    for (i, &(num_i, den_i)) in rates.iter().enumerate() {
        for (j, &(num_j, den_j)) in rates.iter().enumerate().skip(i + 1) {
            let price_ratio = prices[i] / prices[j];
            let expected_ratio = (num_i * den_j) as f64 / (num_j * den_i) as f64;
            let relative_err = (price_ratio - expected_ratio).abs() / expected_ratio;
            assert!(
                relative_err < 0.01,
                "price ratio between ({num_i}/{den_i}) and ({num_j}/{den_j}) should match rate \
                 ratio: got {price_ratio:.6}, expected {expected_ratio:.6}",
            );
        }
    }

    // Submit a quote for each wrapper token to verify the full pipeline works
    // end-to-end (pricing + routing through the seeded Uni V2 pools).
    for (wrapper, &(num, den)) in wrappers.iter().zip(rates) {
        wrapper.mint(trader.address(), 100u64.eth()).await;
        ERC20::Instance::new(*wrapper.address(), web3.provider.clone())
            .approve(onchain.contracts().allowance, 100u64.eth())
            .from(trader.address())
            .send_and_watch()
            .await
            .unwrap();

        let quote = services
            .submit_quote(&OrderQuoteRequest {
                from: trader.address(),
                sell_token: *wrapper.address(),
                buy_token: WETH,
                side: OrderQuoteSide::Sell {
                    sell_amount: SellAmount::BeforeFee {
                        value: (10u64.eth()).try_into().unwrap(),
                    },
                },
                ..Default::default()
            })
            .await;

        assert!(
            quote.is_ok(),
            "quote for wrapper ({num}/{den}) should succeed: {:?}",
            quote.err()
        );
    }
}
