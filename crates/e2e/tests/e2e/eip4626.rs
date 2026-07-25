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
    model::{
        order::BUY_ETH_ADDRESS,
        quote::{OrderQuoteRequest, OrderQuoteSide, SellAmount},
    },
    number::units::EthUnit,
    price_estimation::{
        HEALTHY_PRICE_ESTIMATION_TIME,
        native::{Eip4626, NativePriceEstimateResult, NativePriceEstimating},
    },
    shared::web3::Web3,
    std::time::Duration,
    testlib::tokens::GNO,
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

/// DAI on mainnet — 18-decimal counterpart to USDC for testing the
/// `6-decimal vault wrapping 18-decimal asset` direction.
const DAI: Address = address!("6B175474E89094C44Da98b954EedeAC495271d0F");

/// wmtUSDC on mainnet — a partial EIP-4626 implementation that exposes
/// `asset()` but reverts on `convertToAssets()`. Must classify as non-vault.
const WMT_USDC: Address = address!("C9499006a149C553d18171747ED19Aa7C6Dd19E2");

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

/// Vault and underlying decimals must be threaded through `conversion_rate`
/// correctly in both directions. Native prices are quoted per *atom*, so a
/// wrapper whose whole-share rate is 1:1 with its underlying must still be
/// priced differently when their decimals differ — by exactly
/// `10^(asset_decimals - vault_decimals)`. Exercises both asymmetries:
///   - 18-decimal vault wrapping 6-decimal USDC → per-atom factor = `10^-12`
///     (lands the vault in DAI's `~10^-4` wei/atom range, not USDC's `~10^8`).
///   - 6-decimal vault wrapping 18-decimal DAI → per-atom factor = `10^12`
///     (vice versa).
#[tokio::test]
#[ignore]
async fn forked_node_mainnet_eip4626_decimal_mismatch_native_price() {
    run_forked_test_with_block_number(
        eip4626_decimal_mismatch_native_price_test,
        std::env::var("FORK_URL_MAINNET")
            .expect("FORK_URL_MAINNET must be set to run forked tests"),
        FORK_BLOCK_MAINNET,
    )
    .await;
}

async fn eip4626_decimal_mismatch_native_price_test(web3: Web3) {
    let mut onchain = OnchainComponents::deployed(web3.clone()).await;

    let [solver] = onchain.make_solvers_forked(1u64.eth()).await;

    // `MockERC4626Wrapper` implements `convertToAssets(shares) = shares * num
    // / den`. Setting `num` and `den` to the atom-count of one whole asset
    // and one whole vault token (respectively) yields a 1:1 *whole-token*
    // rate — `convertToAssets(one_whole_vault) = one_whole_asset`.
    let one_whole_6_dec = 1u64.matom(); // 10^6 atoms = 1 whole 6-decimal token
    let one_whole_18_dec = 1u64.eth(); //  10^18 atoms = 1 whole 18-decimal token

    // 18-decimal wrapper of 6-decimal USDC at a 1:1 whole-token rate.
    // `convertToAssets(10^18) = 10^18 * 10^6 / 10^18 = 10^6` (= 1 whole USDC
    // in atoms), so the per-atom factor is `10^-12`.
    let wrapper_18_over_6 = contracts::test::MockERC4626Wrapper::Instance::deploy(
        web3.provider.clone(),
        USDC,
        18u8,
        one_whole_6_dec,  // num: atoms in 1 whole asset (USDC)
        one_whole_18_dec, // den: atoms in 1 whole vault share
    )
    .await
    .unwrap();
    let wrapper_18_over_6_addr = *wrapper_18_over_6.address();

    // 6-decimal wrapper of 18-decimal DAI at a 1:1 whole-token rate.
    // `convertToAssets(10^6) = 10^6 * 10^18 / 10^6 = 10^18` (= 1 whole DAI in
    // atoms), so the per-atom factor is `10^12`.
    let wrapper_6_over_18 = contracts::test::MockERC4626Wrapper::Instance::deploy(
        web3.provider.clone(),
        DAI,
        6u8,
        one_whole_18_dec, // num: atoms in 1 whole asset (DAI)
        one_whole_6_dec,  // den: atoms in 1 whole vault share
    )
    .await
    .unwrap();
    let wrapper_6_over_18_addr = *wrapper_6_over_18.address();

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

    let fetch_price = async |addr: &Address, label: &str| -> f64 {
        wait_for_condition(TIMEOUT, || async {
            services.get_native_price(addr).await.is_ok()
        })
        .await
        .unwrap_or_else(|_| panic!("native price for {label} should be available"));
        services.get_native_price(addr).await.unwrap().price
    };

    let usdc_price = fetch_price(&USDC, "USDC").await;
    let dai_price = fetch_price(&DAI, "DAI").await;

    // 18→6: wrapper is priced per-atom like an 18-decimal stablecoin (DAI's
    // range), so `wrapper_price / usdc_price ≈ 10^-12`.
    let wrapper_18_over_6_price = fetch_price(&wrapper_18_over_6_addr, "18→6 wrapper").await;
    let ratio = wrapper_18_over_6_price / usdc_price;
    assert!(
        (ratio - 1e-12).abs() / 1e-12 < 0.01,
        "18→6 wrapper / USDC ratio should be 1e-12, got {ratio:e}",
    );

    // 6→18: wrapper is priced per-atom like a 6-decimal stablecoin (USDC's
    // range), so `wrapper_price / dai_price ≈ 10^12`.
    let wrapper_6_over_18_price = fetch_price(&wrapper_6_over_18_addr, "6→18 wrapper").await;
    let ratio = wrapper_6_over_18_price / dai_price;
    assert!(
        (ratio - 1e12).abs() / 1e12 < 0.01,
        "6→18 wrapper / DAI ratio should be 1e12, got {ratio:e}",
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

    for token in [BUY_ETH_ADDRESS, USDC, GNO] {
        let price = estimator
            .estimate_native_price(token, HEALTHY_PRICE_ESTIMATION_TIME)
            .await
            .expect("empty-revert on terminal token must not abort the unwrap");
        assert_eq!(price, expected_price);
    }
}

#[tokio::test]
#[ignore]
async fn forked_node_mainnet_eip4626_partial_vault_terminal_token() {
    run_forked_test(
        eip4626_partial_vault_terminal_token_test,
        std::env::var("FORK_URL_MAINNET")
            .expect("FORK_URL_MAINNET must be set to run forked tests"),
    )
    .await;
}

async fn eip4626_partial_vault_terminal_token_test(web3: Web3) {
    let expected_price = 0.0001;
    let inner = FixedPrice(expected_price);
    let estimator = Eip4626::new(Box::new(inner), web3.provider);

    let price = estimator
        .estimate_native_price(WMT_USDC, HEALTHY_PRICE_ESTIMATION_TIME)
        .await
        .expect("token missing convertToAssets() must classify as non-vault, not abort");
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
