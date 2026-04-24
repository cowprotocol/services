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
    futures::{FutureExt, future::BoxFuture},
    model::quote::{OrderQuoteRequest, OrderQuoteSide, SellAmount},
    number::units::EthUnit,
    price_estimation::{
        HEALTHY_PRICE_ESTIMATION_TIME,
        native::{Eip4626, NativePriceEstimateResult, NativePriceEstimating},
    },
    shared::web3::Web3,
    std::time::Duration,
};

/// The block number from which we will fetch state for the forked test.
const FORK_BLOCK_MAINNET: u64 = 23112197;

/// sDAI (Savings DAI) – an EIP-4626 vault wrapping DAI.
const SDAI: Address = address!("83F20F44975D03b1b09e64809B757c47f942BEeA");

/// sDAI whale at [`FORK_BLOCK_MAINNET`].
const SDAI_WHALE: Address = address!("4C612E3B15b96Ff9A6faED838F8d07d479a8dD4c");

/// WETH on mainnet.
const WETH: Address = address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");

/// USDC on mainnet. The proxy has no `asset()` selector, so calls to it
/// revert with *empty* revert data — the regression case below.
const USDC: Address = address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");

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

    let driver_url: url::Url = "http://localhost:11088/test_solver".parse().unwrap();
    let autopilot_config = Configuration {
        native_price_estimation: NativePriceConfig {
            eip4626: true,
            estimators: NativePriceEstimators::new(vec![vec![NativePriceEstimator::driver(
                "test_quoter".to_string(),
                driver_url,
            )]]),
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

/// Tests pricing of mock EIP-4626 vaults with non-trivial conversion rates.
/// Deploys wrapper vaults on top of sDAI (which itself wraps DAI) with
/// different rates and verifies native prices are proportional to their
/// conversion rates.
async fn eip4626_recursive_native_price_test(web3: Web3) {
    let mut onchain = OnchainComponents::deployed(web3.clone()).await;

    let [solver] = onchain.make_solvers_forked(1u64.eth()).await;

    // Deploy mock EIP-4626 vaults wrapping sDAI with different conversion rates.
    // Each wrapper applies `convertToAssets(shares) = shares * num / den`, so a
    // (3, 2) wrapper means 1 share = 1.5 sDAI, making it 1.5x the sDAI price.
    let rates: &[(u64, u64)] = &[(3, 2), (2, 1), (1, 3)];
    let mut wrapper_addrs = Vec::with_capacity(rates.len());
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
        wrapper_addrs.push(*wrapper.address());
    }

    let driver_url: url::Url = "http://localhost:11088/test_solver".parse().unwrap();
    let autopilot_config = Configuration {
        native_price_estimation: NativePriceConfig {
            eip4626: true,
            estimators: NativePriceEstimators::new(vec![vec![NativePriceEstimator::driver(
                "test_quoter".to_string(),
                driver_url,
            )]]),
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

    // Verify native prices: use the first wrapper (3/2) as a baseline and
    // check that the others are priced proportionally to their conversion rate.
    let baseline_addr = wrapper_addrs[0];
    wait_for_condition(TIMEOUT, || async {
        services.get_native_price(&baseline_addr).await.is_ok()
    })
    .await
    .expect("native price for wrapper (3/2) should be available");
    let baseline_price = services
        .get_native_price(&baseline_addr)
        .await
        .unwrap()
        .price;

    // Wrapper (2/1) has rate 2/1 vs baseline 3/2, so its price should be
    // (2/1) / (3/2) = 4/3 of the baseline.
    let addr = wrapper_addrs[1];
    wait_for_condition(TIMEOUT, || async {
        services.get_native_price(&addr).await.is_ok()
    })
    .await
    .expect("native price for wrapper (2/1) should be available");
    let price = services.get_native_price(&addr).await.unwrap().price;
    let ratio = price / baseline_price;
    assert!(
        (ratio - 4.0 / 3.0).abs() / (4.0 / 3.0) < 0.01,
        "wrapper (2/1) price ratio to baseline (3/2) should be 4/3: got {ratio:.6}",
    );

    // Wrapper (1/3) has rate 1/3 vs baseline 3/2, so its price should be
    // (1/3) / (3/2) = 2/9 of the baseline.
    let addr = wrapper_addrs[2];
    wait_for_condition(TIMEOUT, || async {
        services.get_native_price(&addr).await.is_ok()
    })
    .await
    .expect("native price for wrapper (1/3) should be available");
    let price = services.get_native_price(&addr).await.unwrap().price;
    let ratio = price / baseline_price;
    assert!(
        (ratio - 2.0 / 9.0).abs() / (2.0 / 9.0) < 0.01,
        "wrapper (1/3) price ratio to baseline (3/2) should be 2/9: got {ratio:.6}",
    );
}

/// Regression: tokens that revert `asset()` with empty data (e.g. USDC) must
/// classify as non-vault, not abort as a transport failure.
#[tokio::test]
#[ignore]
async fn forked_node_mainnet_eip4626_empty_revert_terminal_token() {
    run_forked_test_with_block_number(
        eip4626_empty_revert_terminal_token_test,
        std::env::var("FORK_URL_MAINNET")
            .expect("FORK_URL_MAINNET must be set to run forked tests"),
        FORK_BLOCK_MAINNET,
    )
    .await;
}

async fn eip4626_empty_revert_terminal_token_test(web3: Web3) {
    // USDC is expected to classify as non-vault, so `inner`'s price round-trips
    // unchanged — any fixed value works.
    let expected_price = 0.0001;
    let inner = FixedPrice(expected_price);
    let estimator = Eip4626::new(Box::new(inner), web3.provider);
    let price = estimator
        .estimate_native_price(USDC, HEALTHY_PRICE_ESTIMATION_TIME)
        .await
        .expect("empty-revert on terminal token must not abort the unwrap");
    assert_eq!(price, expected_price);
}

struct FixedPrice(f64);

impl NativePriceEstimating for FixedPrice {
    fn estimate_native_price(
        &self,
        _token: Address,
        _timeout: Duration,
    ) -> BoxFuture<'_, NativePriceEstimateResult> {
        let price = self.0;
        async move { Ok(price) }.boxed()
    }
}
