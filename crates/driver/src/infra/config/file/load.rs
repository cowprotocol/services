use {
    crate::{
        domain::eth,
        infra::{self, blockchain, config::file, liquidity, mempool, simulator, solver},
    },
    futures::future::join_all,
    lazy_static::lazy_static,
    reqwest::Url,
    std::path::Path,
    tokio::fs,
};

lazy_static! {
    pub static ref DEFAULT_GRAPH_API_BASE_URL: Url =
        Url::parse("https://api.thegraph.com/subgraphs/name/")
            .expect("invalid default Graph API base URL");
}

/// Load the driver configuration from a TOML file for the specifed Ethereum
/// network.
///
/// # Panics
///
/// This method panics if the config is invalid or on I/O errors.
pub async fn load(chain: eth::ChainId, path: &Path) -> infra::Config {
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
        config.chain_id.map(eth::ChainId).unwrap_or(chain),
        chain,
        "The configured chain ID does not match connected Ethereum node"
    );
    let graph_api_base_url = config
        .liquidity
        .graph_api_base_url
        .unwrap_or(DEFAULT_GRAPH_API_BASE_URL.clone());
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
                    relative: config.slippage.relative,
                    absolute: config.slippage.absolute.map(eth::Ether),
                },
                liquidity: if config.skip_liquidity {
                    solver::Liquidity::Skip
                } else {
                    solver::Liquidity::Fetch
                },
                account,
                merging: if config.skip_merge {
                    solver::Merging::Skip
                } else {
                    solver::Merging::Fetch
                },
                timeouts: solver::Timeouts {
                    http_delay: chrono::Duration::from_std(config.timeouts.http_time_buffer)
                        .unwrap(),
                    solving_share_of_deadline: config
                        .timeouts
                        .solving_share_of_deadline
                        .try_into()
                        .unwrap(),
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
                    } => liquidity::config::UniswapV3 {
                        max_pools_to_initialize,
                        ..match preset {
                            file::UniswapV3Preset::UniswapV3 => {
                                liquidity::config::UniswapV3::uniswap_v3(&graph_api_base_url, chain)
                            }
                        }
                        .expect("no Uniswap V3 preset for current network")
                    },
                    file::UniswapV3Config::Manual {
                        router,
                        max_pools_to_initialize,
                    } => liquidity::config::UniswapV3 {
                        router: router.into(),
                        max_pools_to_initialize,
                        graph_api_base_url: graph_api_base_url.clone(),
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
                    } => liquidity::config::BalancerV2 {
                        pool_deny_list: pool_deny_list.clone(),
                        ..match preset {
                            file::BalancerV2Preset::BalancerV2 => {
                                liquidity::config::BalancerV2::balancer_v2(
                                    &graph_api_base_url,
                                    chain,
                                )
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
                        graph_api_base_url: graph_api_base_url.clone(),
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
                max_confirm_time: config.submission.max_confirm_time,
                retry_interval: config.submission.retry_interval,
                kind: match mempool {
                    file::Mempool::Public => {
                        // If there is no private mempool, revert protection is
                        // disabled, otherwise driver would not even try to settle revertable
                        // settlements
                        mempool::Kind::Public(
                            if config
                                .submission
                                .mempools
                                .iter()
                                .any(|pool| matches!(pool, file::Mempool::MevBlocker { .. }))
                            {
                                mempool::RevertProtection::Enabled
                            } else {
                                mempool::RevertProtection::Disabled
                            },
                        )
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
                submission: match config.submission.logic {
                    file::Logic::Boundary => mempool::SubmissionLogic::Boundary,
                    file::Logic::Native => mempool::SubmissionLogic::Native,
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
        },
        disable_access_list_simulation: config.disable_access_list_simulation,
        disable_gas_simulation: config.disable_gas_simulation.map(Into::into),
    }
}
