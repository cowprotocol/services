use {
    crate::{
        domain::{competition::bad_tokens, eth},
        infra::{
            self,
            blockchain,
            config::file,
            liquidity,
            mempool,
            simulator,
            solver::{self, BadTokenDetection, SolutionMerging},
        },
    },
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
        solvers: join_all(config.solvers.into_iter().map(|config| async move {
            let account = match config.account {
                file::Account::PrivateKey(private_key) => ethcontract::Account::Offline(
                    ethcontract::PrivateKey::from_raw(private_key.0).unwrap(),
                    None,
                ),
                file::Account::Kms(key_id) => {
                    let config = ethcontract::aws_config::load_from_env().await;
                    let account =
                        ethcontract::transaction::kms::Account::new((&config).into(), &key_id.0)
                            .await
                            .unwrap_or_else(|_| panic!("Unable to load KMS account {:?}", key_id));
                    ethcontract::Account::Kms(account, None)
                }
                file::Account::Address(address) => ethcontract::Account::Local(address, None),
            };
            solver::Config {
                endpoint: config.endpoint,
                name: config.name.into(),
                slippage: solver::Slippage {
                    relative: big_decimal_to_big_rational(&config.slippage.relative),
                    absolute: config.slippage.absolute.map(eth::Ether),
                },
                liquidity: if config.skip_liquidity {
                    solver::Liquidity::Skip
                } else {
                    solver::Liquidity::Fetch
                },
                account,
                timeouts: solver::Timeouts {
                    http_delay: chrono::Duration::from_std(config.timeouts.http_time_buffer)
                        .unwrap(),
                    solving_share_of_deadline: config
                        .timeouts
                        .solving_share_of_deadline
                        .try_into()
                        .unwrap(),
                },
                request_headers: config.request_headers,
                fee_handler: config.fee_handler,
                quote_using_limit_orders: config.quote_using_limit_orders,
                merge_solutions: match config.merge_solutions {
                    true => SolutionMerging::Allowed,
                    false => SolutionMerging::Forbidden,
                },
                s3: config.s3.map(Into::into),
                solver_native_token: config.manage_native_token.to_domain(),
                quote_tx_origin: config.quote_tx_origin.map(eth::Address),
                response_size_limit_max_bytes: config.response_size_limit_max_bytes,
                bad_token_detection: BadTokenDetection {
                    tokens_supported: config
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
                    enable_simulation_strategy: config
                        .bad_token_detection
                        .enable_simulation_strategy,
                    enable_metrics_strategy: config.bad_token_detection.enable_metrics_strategy,
                    metrics_strategy_failure_ratio: config
                        .bad_token_detection
                        .metrics_strategy_failure_ratio,
                    metrics_strategy_required_measurements: config
                        .bad_token_detection
                        .metrics_strategy_required_measurements,
                    metrics_strategy_log_only: config.bad_token_detection.metrics_strategy_log_only,
                    metrics_strategy_token_freeze_time: config
                        .bad_token_detection
                        .metrics_strategy_token_freeze_time,
                },
                settle_queue_size: config.settle_queue_size,
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
                    } => liquidity::config::UniswapV3 {
                        max_pools_to_initialize,
                        ..match preset {
                            file::UniswapV3Preset::UniswapV3 => {
                                liquidity::config::UniswapV3::uniswap_v3(&graph_url, chain)
                            }
                        }
                        .expect("no Uniswap V3 preset for current network")
                    },
                    file::UniswapV3Config::Manual {
                        router,
                        max_pools_to_initialize,
                        graph_url,
                    } => liquidity::config::UniswapV3 {
                        router: router.into(),
                        max_pools_to_initialize,
                        graph_url,
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
                    } => liquidity::config::BalancerV2 {
                        pool_deny_list: pool_deny_list.clone(),
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
                    } => liquidity::config::BalancerV2 {
                        vault: vault.into(),
                        weighted: weighted
                            .into_iter()
                            .map(eth::ContractAddress::from)
                            .collect(),
                        weighted_v3plus: weighted_v3plus
                            .into_iter()
                            .map(eth::ContractAddress::from)
                            .collect(),
                        stable: stable.into_iter().map(eth::ContractAddress::from).collect(),
                        liquidity_bootstrapping: liquidity_bootstrapping
                            .into_iter()
                            .map(eth::ContractAddress::from)
                            .collect(),
                        composable_stable: composable_stable
                            .into_iter()
                            .map(eth::ContractAddress::from)
                            .collect(),
                        pool_deny_list: pool_deny_list.clone(),
                        graph_url,
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
        mempools: config
            .submission
            .mempools
            .iter()
            .map(|mempool| mempool::Config {
                min_priority_fee: config.submission.min_priority_fee,
                gas_price_cap: config.submission.gas_price_cap,
                target_confirm_time: config.submission.target_confirm_time,
                retry_interval: config.submission.retry_interval,
                kind: match mempool {
                    file::Mempool::Public {
                        max_additional_tip,
                        additional_tip_percentage,
                    } => {
                        // If there is no private mempool, revert protection is
                        // disabled, otherwise driver would not even try to settle revertable
                        // settlements
                        let revert_protection = if config
                            .submission
                            .mempools
                            .iter()
                            .any(|pool| matches!(pool, file::Mempool::MevBlocker { .. }))
                        {
                            mempool::RevertProtection::Enabled
                        } else {
                            mempool::RevertProtection::Disabled
                        };

                        mempool::Kind::Public {
                            max_additional_tip: *max_additional_tip,
                            additional_tip_percentage: *additional_tip_percentage,
                            revert_protection,
                        }
                    }
                    file::Mempool::MevBlocker {
                        url,
                        max_additional_tip,
                        additional_tip_percentage,
                        use_soft_cancellations,
                    } => mempool::Kind::MEVBlocker {
                        url: url.to_owned(),
                        max_additional_tip: *max_additional_tip,
                        additional_tip_percentage: *additional_tip_percentage,
                        use_soft_cancellations: *use_soft_cancellations,
                    },
                },
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
            cow_amms: config
                .contracts
                .cow_amms
                .into_iter()
                .map(|cfg| blockchain::contracts::CowAmmConfig {
                    index_start: cfg.index_start,
                    factory: cfg.factory,
                    helper: cfg.helper,
                })
                .collect(),
        },
        disable_access_list_simulation: config.disable_access_list_simulation,
        disable_gas_simulation: config.disable_gas_simulation.map(Into::into),
        gas_estimator: config.gas_estimator,
        order_priority_strategies: config.order_priority_strategies,
        archive_node_url: config.archive_node_url,
        simulation_bad_token_max_age: config.simulation_bad_token_max_age,
        simulation_target: config.simulation_target,
    }
}
