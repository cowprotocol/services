use {
    crate::{
        domain::eth,
        infra::{self, config::file, liquidity, mempool, simulator, solver},
    },
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
    let config: file::Config = toml::de::from_str(&data)
        .unwrap_or_else(|e| panic!("TOML syntax error while reading {path:?}: {e:?}"));
    infra::Config {
        solvers: config
            .solvers
            .into_iter()
            .map(|config| solver::Config {
                endpoint: config.endpoint,
                name: config.name.into(),
                slippage: solver::Slippage {
                    relative: config.relative_slippage,
                    absolute: config.absolute_slippage.map(Into::into),
                },
                private_key: eth::PrivateKey::from_raw(config.private_key.0).unwrap(),
            })
            .collect(),
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
                    } => mempool::Kind::Flashbots {
                        url: url.to_owned(),
                        max_additional_tip: *max_additional_tip,
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
    }
}
