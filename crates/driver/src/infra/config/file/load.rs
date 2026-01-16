use {
    crate::{
        domain::{competition::bad_tokens, eth},
        infra::{
            self,
            blockchain,
            config::file,
            liquidity,
            mempool,
            notify,
            simulator,
            solver::{self, Account, BadTokenDetection, SolutionMerging},
        },
    },
    alloy::signers::{aws::AwsSigner, local::PrivateKeySigner},
    chain::Chain,
    futures::future::join_all,
    number::conversions::big_decimal_to_big_rational,
    std::path::Path,
    tokio::fs,
};

/// Load the driver configuration from a TOML file for the specifed Ethereum
/// network.
///
/// # Panics
///
/// This method panics if the config is invalid or on I/O errors.
pub async fn load(chain: Chain, path: &Path) -> infra::Config {
    let data = fs::read_to_string(path)
        .await
        .unwrap_or_else(|e| panic!("I/O error while reading {path:?}: {e:?}"));

    let config: file::Config = toml::de::from_str(&data).unwrap_or_else(|err| {
        if std::env::var("TOML_TRACE_ERROR").is_ok_and(|v| v == "1") {
            panic!("failed to parse TOML config at {path:?}: {err:#?}")
        } else {
            panic!(
                "failed to parse TOML config at: {path:?}. Set TOML_TRACE_ERROR=1 to print \
                 parsing error but this may leak secrets."
            )
        }
    });

    assert_eq!(
        config
            .chain_id
            .map(|id| Chain::try_from(id).expect("unsupported chain ID"))
            .unwrap_or(chain),
        chain,
        "The configured chain ID does not match the connected Ethereum node"
    );
    infra::Config {
        solvers: join_all(config.solvers.into_iter().map(|solver_config| async move {
            let account: Account = match solver_config.account {
                file::Account::PrivateKey(private_key) => {
                    PrivateKeySigner::from_bytes(&private_key)
                        .expect(
                            "private key should
                                            be valid",
                        )
                        .into()
                }
                file::Account::Kms(arn) => {
                    let sdk_config = alloy::signers::aws::aws_config::load_from_env().await;
                    let client = alloy::signers::aws::aws_sdk_kms::Client::new(&sdk_config);
                    AwsSigner::new(client, arn.0, config.chain_id)
                        .await
                        .expect("unable to load kms account {arn:?}")
                        .into()
                }
                file::Account::Address(address) => Account::Address(address),
            };
            solver::Config {
                endpoint: solver_config.endpoint,
                name: solver_config.name.into(),
                slippage: solver::Slippage {
                    relative: big_decimal_to_big_rational(&solver_config.slippage.relative),
                    absolute: solver_config.slippage.absolute.map(eth::Ether),
                },
                liquidity: if solver_config.skip_liquidity {
                    solver::Liquidity::Skip
                } else {
                    solver::Liquidity::Fetch
                },
                account,
                timeouts: solver::Timeouts {
                    http_delay: chrono::Duration::from_std(solver_config.timeouts.http_time_buffer)
                        .unwrap(),
                    solving_share_of_deadline: solver_config
                        .timeouts
                        .solving_share_of_deadline
                        .try_into()
                        .unwrap(),
                },
                request_headers: solver_config.request_headers,
                fee_handler: solver_config.fee_handler,
                quote_using_limit_orders: solver_config.quote_using_limit_orders,
                merge_solutions: match solver_config.merge_solutions {
                    true => SolutionMerging::Allowed {
                        max_orders_per_merged_solution: solver_config
                            .max_orders_per_merged_solution,
                    },
                    false => SolutionMerging::Forbidden,
                },
                s3: solver_config.s3.map(Into::into),
                solver_native_token: solver_config.manage_native_token.to_domain(),
                quote_tx_origin: solver_config.quote_tx_origin,
                response_size_limit_max_bytes: solver_config.response_size_limit_max_bytes,
                bad_token_detection: BadTokenDetection {
                    tokens_supported: solver_config
                        .bad_token_detection
                        .token_supported
                        .iter()
                        .map(|(token, supported)| {
                            (
                                eth::TokenAddress(eth::ContractAddress(*token)),
                                match supported {
                                    true => bad_tokens::Quality::Supported,
                                    false => bad_tokens::Quality::Unsupported,
                                },
                            )
                        })
                        .collect(),
                    enable_simulation_strategy: solver_config
                        .bad_token_detection
                        .enable_simulation_strategy,
                    enable_metrics_strategy: solver_config
                        .bad_token_detection
                        .enable_metrics_strategy,
                    metrics_strategy_failure_ratio: solver_config
                        .bad_token_detection
                        .metrics_strategy_failure_ratio,
                    metrics_strategy_required_measurements: solver_config
                        .bad_token_detection
                        .metrics_strategy_required_measurements,
                    metrics_strategy_log_only: solver_config
                        .bad_token_detection
                        .metrics_strategy_log_only,
                    metrics_strategy_token_freeze_time: solver_config
                        .bad_token_detection
                        .metrics_strategy_token_freeze_time,
                },
                settle_queue_size: solver_config.settle_queue_size,
                flashloans_enabled: config.flashloans_enabled,
                fetch_liquidity_at_block: match config.liquidity.fetch_at_block {
                    file::AtBlock::Latest => liquidity::AtBlock::Latest,
                    file::AtBlock::Finalized => liquidity::AtBlock::Finalized,
                },
            }
        }))
        .await,
        liquidity: liquidity::Config {
            base_tokens: config
                .liquidity
                .base_tokens
                .iter()
                .copied()
                .map(eth::TokenAddress::from)
                .collect(),
            uniswap_v2: config
                .liquidity
                .uniswap_v2
                .iter()
                .cloned()
                .map(|config| match config {
                    file::UniswapV2Config::Preset { preset } => match preset {
                        file::UniswapV2Preset::UniswapV2 => {
                            liquidity::config::UniswapV2::uniswap_v2(chain)
                        }
                        file::UniswapV2Preset::SushiSwap => {
                            liquidity::config::UniswapV2::sushi_swap(chain)
                        }
                        file::UniswapV2Preset::Honeyswap => {
                            liquidity::config::UniswapV2::honeyswap(chain)
                        }
                        file::UniswapV2Preset::Baoswap => {
                            liquidity::config::UniswapV2::baoswap(chain)
                        }
                        file::UniswapV2Preset::PancakeSwap => {
                            liquidity::config::UniswapV2::pancake_swap(chain)
                        }
                        file::UniswapV2Preset::TestnetUniswapV2 => {
                            liquidity::config::UniswapV2::testnet_uniswapv2(chain)
                        }
                    }
                    .expect("no Uniswap V2 preset for current network"),
                    file::UniswapV2Config::Manual {
                        router,
                        pool_code,
                        missing_pool_cache_time,
                    } => liquidity::config::UniswapV2 {
                        router: router.into(),
                        pool_code: pool_code.into(),
                        missing_pool_cache_time,
                    },
                })
                .collect(),
            swapr: config
                .liquidity
                .swapr
                .iter()
                .cloned()
                .map(|config| match config {
                    file::SwaprConfig::Preset { preset } => match preset {
                        file::SwaprPreset::Swapr => liquidity::config::Swapr::swapr(chain),
                    }
                    .expect("no Swapr preset for current network"),
                    file::SwaprConfig::Manual {
                        router,
                        pool_code,
                        missing_pool_cache_time,
                    } => liquidity::config::Swapr {
                        router: router.into(),
                        pool_code: pool_code.into(),
                        missing_pool_cache_time,
                    },
                })
                .collect(),
            uniswap_v3: config
                .liquidity
                .uniswap_v3
                .iter()
                .cloned()
                .map(|config| match config {
                    file::UniswapV3Config::Preset {
                        preset,
                        max_pools_to_initialize,
                        graph_url,
                        reinit_interval,
                        max_pools_per_tick_query,
                    } => liquidity::config::UniswapV3 {
                        max_pools_to_initialize,
                        reinit_interval,
                        ..match preset {
                            file::UniswapV3Preset::UniswapV3 => {
                                liquidity::config::UniswapV3::uniswap_v3(
                                    &graph_url,
                                    chain,
                                    max_pools_per_tick_query,
                                )
                            }
                        }
                        .expect("no Uniswap V3 preset for current network")
                    },
                    file::UniswapV3Config::Manual {
                        router,
                        max_pools_to_initialize,
                        graph_url,
                        reinit_interval,
                        max_pools_per_tick_query,
                    } => liquidity::config::UniswapV3 {
                        router: router.into(),
                        max_pools_to_initialize,
                        graph_url,
                        reinit_interval,
                        max_pools_per_tick_query,
                    },
                })
                .collect(),
            balancer_v2: config
                .liquidity
                .balancer_v2
                .iter()
                .cloned()
                .map(|config| match config {
                    file::BalancerV2Config::Preset {
                        preset,
                        pool_deny_list,
                        graph_url,
                        reinit_interval,
                    } => liquidity::config::BalancerV2 {
                        pool_deny_list: pool_deny_list.clone(),
                        reinit_interval,
                        ..match preset {
                            file::BalancerV2Preset::BalancerV2 => {
                                liquidity::config::BalancerV2::balancer_v2(&graph_url, chain)
                            }
                        }
                        .expect("no Balancer V2 preset for current network")
                    },
                    file::BalancerV2Config::Manual {
                        vault,
                        weighted,
                        weighted_v3plus,
                        stable,
                        liquidity_bootstrapping,
                        composable_stable,
                        pool_deny_list,
                        graph_url,
                        reinit_interval,
                    } => liquidity::config::BalancerV2 {
                        vault: vault.into(),
                        weighted,
                        weighted_v3plus,
                        stable,
                        liquidity_bootstrapping,
                        composable_stable,
                        pool_deny_list: pool_deny_list.clone(),
                        graph_url,
                        reinit_interval,
                    },
                })
                .collect(),
            zeroex: config
                .liquidity
                .zeroex
                .map(|config| liquidity::config::ZeroEx {
                    base_url: config.base_url,
                    api_key: config.api_key,
                    http_timeout: config.http_timeout,
                }),
        },
        liquidity_sources_notifier: config.liquidity_sources_notifier.map(|notifier| {
            notify::liquidity_sources::config::Config {
                liquorice: notifier.liquorice.map(|liquorice_config| {
                    notify::liquidity_sources::config::Liquorice {
                        base_url: liquorice_config.base_url,
                        api_key: liquorice_config.api_key,
                        http_timeout: liquorice_config.http_timeout,
                    }
                }),
            }
        }),
        mempools: config
            .submission
            .mempools
            .iter()
            .enumerate()
            .map(|(index, mempool)| mempool::Config {
                min_priority_fee: config.submission.min_priority_fee,
                gas_price_cap: config.submission.gas_price_cap,
                target_confirm_time: config.submission.target_confirm_time,
                retry_interval: config.submission.retry_interval,
                nonce_block_number: config.submission.nonce_block_number.map(Into::into),
                name: mempool
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("mempool_{index}")),
                url: mempool.url.clone(),
                revert_protection: match mempool.mines_reverting_txs {
                    true => mempool::RevertProtection::Disabled,
                    false => mempool::RevertProtection::Enabled,
                },
                max_additional_tip: mempool.max_additional_tip,
                additional_tip_percentage: mempool.additional_tip_percentage,
            })
            .collect(),
        simulator: match (config.tenderly, config.enso) {
            (Some(config), None) => {
                Some(simulator::Config::Tenderly(simulator::tenderly::Config {
                    url: config.url,
                    api_key: config.api_key,
                    user: config.user,
                    project: config.project,
                    save: config.save,
                    save_if_fails: config.save_if_fails,
                }))
            }
            (None, Some(config)) => Some(simulator::Config::Enso(simulator::enso::Config {
                url: config.url,
                network_block_interval: config.network_block_interval,
            })),
            (None, None) => None,
            (Some(_), Some(_)) => panic!("Cannot configure both Tenderly and Enso"),
        },
        contracts: blockchain::contracts::Addresses {
            settlement: config.contracts.gp_v2_settlement.map(Into::into),
            weth: config.contracts.weth.map(Into::into),
            balances: config.contracts.balances.map(Into::into),
            signatures: config.contracts.signatures.map(Into::into),
            cow_amm_helper_by_factory: config
                .contracts
                .cow_amms
                .into_iter()
                .map(|cfg| (cfg.factory.into(), cfg.helper.into()))
                .collect(),
            flashloan_router: config.contracts.flashloan_router.map(Into::into),
        },
        disable_access_list_simulation: config.disable_access_list_simulation,
        disable_gas_simulation: config.disable_gas_simulation.map(Into::into),
        gas_estimator: config.gas_estimator,
        order_priority_strategies: config.order_priority_strategies,
        simulation_bad_token_max_age: config.simulation_bad_token_max_age,
        app_data_fetching: config.app_data_fetching,
        tx_gas_limit: config.tx_gas_limit,
    }
}
