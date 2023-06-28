use {
    crate::{
        domain::eth,
        infra::{self, config::file, liquidity, mempool, simulator, solver},
    },
    futures::future::join_all,
    std::path::Path,
    tokio::fs,
};

/// Load the driver configuration from a TOML file.
///
/// # Panics
///
/// This method panics if the config is invalid or on I/O errors.
pub async fn load(path: &Path) -> infra::Config {
    let data = fs::read_to_string(path)
        .await
        .unwrap_or_else(|e| panic!("I/O error while reading {path:?}: {e:?}"));
    // Not printing detailed error because it could leak private keys.
    let config: file::Config = toml::de::from_str(&data)
        .unwrap_or_else(|_| panic!("TOML syntax error while reading {path:?}"));
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
                .into_iter()
                .map(eth::TokenAddress::from)
                .collect(),
            uniswap_v2: config
                .liquidity
                .uniswap_v2
                .into_iter()
                .map(|config| liquidity::config::UniswapV2 {
                    router: config.router.into(),
                    pool_code: config.pool_code.into(),
                })
                .collect(),
            uniswap_v3: config
                .liquidity
                .uniswap_v3
                .into_iter()
                .map(|config| liquidity::config::UniswapV3 {
                    router: config.router.into(),
                    max_pools_to_initialize: config.max_pools_to_initialize,
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
        contracts: config.contracts,
        disable_access_list_simulation: config.disable_access_list_simulation,
        disable_gas_simulation: config.disable_gas_simulation.map(Into::into),
    }
}
