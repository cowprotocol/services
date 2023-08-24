use {
    crate::{
        domain::eth,
        infra::{self, blockchain, config::file, liquidity, mempool, simulator, solver},
    },
    futures::future::join_all,
    std::path::Path,
    tokio::fs,
};

/// Load the driver configuration from a TOML file for the specifed Ethereum
/// network.
///
/// # Panics
///
/// This method panics if the config is invalid or on I/O errors.
pub async fn load(network: &blockchain::Network, path: &Path) -> infra::Config {
    let data = fs::read_to_string(path)
        .await
        .unwrap_or_else(|e| panic!("I/O error while reading {path:?}: {e:?}"));
    // Not printing detailed error because it could leak private keys.
    let config: file::Config = toml::de::from_str(&data)
        .unwrap_or_else(|_| panic!("TOML syntax error while reading {path:?}"));

    assert_eq!(
        config.chain_id.map(eth::ChainId).unwrap_or(network.chain),
        network.chain,
        "The configured chain ID does not match connected Ethereum node"
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
            };
            solver::Config {
                endpoint: config.endpoint,
                name: config.name.into(),
                slippage: solver::Slippage {
                    relative: config.relative_slippage,
                    absolute: config.absolute_slippage.map(Into::into),
                },
                account,
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
                            liquidity::config::UniswapV2::uniswap_v2(&network.id)
                        }
                        file::UniswapV2Preset::SushiSwap => {
                            liquidity::config::UniswapV2::sushi_swap(&network.id)
                        }
                        file::UniswapV2Preset::Honeyswap => {
                            liquidity::config::UniswapV2::honeyswap(&network.id)
                        }
                        file::UniswapV2Preset::Baoswap => {
                            liquidity::config::UniswapV2::baoswap(&network.id)
                        }
                        file::UniswapV2Preset::PancakeSwap => {
                            liquidity::config::UniswapV2::pancake_swap(&network.id)
                        }
                    }
                    .expect("no Uniswap V2 preset for current network"),
                    file::UniswapV2Config::Manual { router, pool_code } => {
                        liquidity::config::UniswapV2 {
                            router: router.into(),
                            pool_code: pool_code.into(),
                        }
                    }
                })
                .collect(),
            swapr: config
                .liquidity
                .swapr
                .iter()
                .cloned()
                .map(|config| match config {
                    file::SwaprConfig::Preset { preset } => match preset {
                        file::SwaprPreset::Swapr => liquidity::config::Swapr::swapr(&network.id),
                    }
                    .expect("no Swapr preset for current network"),
                    file::SwaprConfig::Manual { router, pool_code } => liquidity::config::Swapr {
                        router: router.into(),
                        pool_code: pool_code.into(),
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
                                liquidity::config::UniswapV3::uniswap_v3(&network.id)
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
                                liquidity::config::BalancerV2::balancer_v2(&network.id)
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
                        pool_deny_list: pool_deny_list.clone(),
                    },
                })
                .collect(),
        },
        mempools: config
            .submission
            .mempools
            .iter()
            .map(|mempool| mempool::Config {
                additional_tip_percentage: config.submission.additional_tip_percentage,
                gas_price_cap: config.submission.gas_price_cap,
                target_confirm_time: std::time::Duration::from_secs(
                    config.submission.target_confirm_time_secs,
                ),
                max_confirm_time: std::time::Duration::from_secs(
                    config.submission.max_confirm_time_secs,
                ),
                retry_interval: std::time::Duration::from_secs(
                    config.submission.retry_interval_secs,
                ),
                kind: match mempool {
                    file::Mempool::Public {
                        disable_high_risk_public_mempool_transactions,
                    } => mempool::Kind::Public(if *disable_high_risk_public_mempool_transactions {
                        mempool::HighRisk::Disabled
                    } else {
                        mempool::HighRisk::Enabled
                    }),
                    file::Mempool::Flashbots {
                        url,
                        max_additional_tip,
                        use_soft_cancellations,
                    } => mempool::Kind::Flashbots {
                        url: url.to_owned(),
                        max_additional_tip: *max_additional_tip,
                        use_soft_cancellations: *use_soft_cancellations,
                    },
                },
            })
            .collect(),
        tenderly: config.tenderly.map(|config| simulator::tenderly::Config {
            url: config.url,
            api_key: config.api_key,
            user: config.user,
            project: config.project,
            save: config.save,
            save_if_fails: config.save_if_fails,
        }),
        contracts: blockchain::contracts::Addresses {
            settlement: config.contracts.gp_v2_settlement.map(Into::into),
            weth: config.contracts.weth.map(Into::into),
        },
        disable_access_list_simulation: config.disable_access_list_simulation,
        disable_gas_simulation: config.disable_gas_simulation.map(Into::into),
    }
}
