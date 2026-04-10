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

    // Configure native price estimation with an EIP-4626 wrapper so that the
    // protocol can price sDAI by looking up its underlying DAI and applying the
    // vault conversion rate.
    let driver_url = "http://localhost:11088/test_solver".parse().unwrap();
    let autopilot_config = Configuration {
        native_price_estimation: NativePriceConfig {
            estimators: NativePriceEstimators::new(vec![vec![
                // Eip4626 wraps the next estimator in the list (test_quoter).
                NativePriceEstimator::Eip4626,
                NativePriceEstimator::driver("test_quoter".to_string(), driver_url),
                // Standalone estimator for non-vault tokens.
                NativePriceEstimator::driver(
                    "test_quoter".to_string(),
                    "http://localhost:11088/test_solver".parse().unwrap(),
                ),
            ]]),
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

/// Tests pricing of recursive EIP-4626 vaults with non-trivial conversion
/// rates. Deploys mock wrapper vaults on top of sDAI (which itself wraps DAI)
/// with different rates and verifies the prices scale correctly.
///
/// Unlike the non-recursive test we cannot submit a full quote because the
/// freshly-deployed wrapper tokens have no DEX liquidity. Instead we verify
/// that the native price endpoint returns correctly scaled prices, which
/// exercises the full Eip4626 → Eip4626 → Driver chain.
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

    // Two chained Eip4626 estimators: the first unwraps the mock wrapper to
    // sDAI, the second unwraps sDAI to DAI, and the Driver prices DAI.
    let driver_url = "http://localhost:11088/test_solver".parse().unwrap();
    let autopilot_config = Configuration {
        native_price_estimation: NativePriceConfig {
            estimators: NativePriceEstimators::new(vec![vec![
                NativePriceEstimator::Eip4626,
                NativePriceEstimator::Eip4626,
                NativePriceEstimator::driver("test_quoter".to_string(), driver_url),
                // Standalone estimator for non-vault tokens.
                NativePriceEstimator::driver(
                    "test_quoter".to_string(),
                    "http://localhost:11088/test_solver".parse().unwrap(),
                ),
            ]]),
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

    // First, get the sDAI native price as a baseline via the native price
    // endpoint. sDAI is priced through the second Eip4626 in the chain.
    wait_for_condition(TIMEOUT, || async {
        services.get_native_price(&SDAI).await.is_ok()
    })
    .await
    .expect("sDAI native price should be available");
    let sdai_price = services.get_native_price(&SDAI).await.unwrap().price;

    // Verify each wrapper's price is the sDAI price scaled by the wrapper's
    // conversion rate. This ensures the rate math is actually applied (a 1:1
    // mock would pass even if the rate were ignored).
    for (&addr, &(num, den)) in wrapper_addrs.iter().zip(rates) {
        wait_for_condition(TIMEOUT, || async {
            services.get_native_price(&addr).await.is_ok()
        })
        .await
        .unwrap_or_else(|_| panic!("native price for wrapper ({num}/{den}) should be available"));

        let wrapper_price = services.get_native_price(&addr).await.unwrap().price;
        let expected_ratio = num as f64 / den as f64;
        let actual_ratio = wrapper_price / sdai_price;

        assert!(
            (actual_ratio - expected_ratio).abs() / expected_ratio < 0.01,
            "wrapper ({num}/{den}): expected ratio ~{expected_ratio:.4}, got {actual_ratio:.4} \
             (wrapper={wrapper_price}, sdai={sdai_price})",
        );
    }
}
