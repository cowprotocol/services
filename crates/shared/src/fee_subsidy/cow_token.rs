use {
    super::{FeeSubsidizing, Subsidy, SubsidyParameters},
    crate::account_balances::{BalanceFetching, Query},
    anyhow::{Context, Result},
    contracts::{CowProtocolToken, CowProtocolVirtualToken},
    model::order::SellTokenSource,
    primitive_types::{H160, U256},
    std::{collections::BTreeMap, sync::Arc},
};

/// Maps how many base units of COW someone must own at least in order to
/// qualify for a given fee subsidy factor.
#[derive(Clone, Debug, Default)]
pub struct SubsidyTiers(BTreeMap<U256, f64>);

impl std::str::FromStr for SubsidyTiers {
    type Err = anyhow::Error;

    fn from_str(serialized: &str) -> Result<Self, Self::Err> {
        let mut tiers = BTreeMap::default();

        for tier in serialized.split(',') {
            let (threshold, fee_factor) = tier
                .split_once(':')
                .with_context(|| format!("too few arguments for subsidy tier in \"{tier}\""))?;

            let threshold: u64 = threshold
                .parse()
                .with_context(|| format!("can not parse threshold \"{threshold}\" as u64"))?;
            let threshold = U256::from(threshold)
                .checked_mul(U256::exp10(18))
                .with_context(|| format!("threshold {threshold} would overflow U256"))?;

            let fee_factor: f64 = fee_factor
                .parse()
                .with_context(|| format!("can not parse fee factor \"{fee_factor}\" as f64"))?;

            anyhow::ensure!(
                (0.0..=1.0).contains(&fee_factor),
                "fee factor must be in the range of [0.0, 1.0]"
            );

            if let Some(_existing) = tiers.insert(threshold, fee_factor) {
                anyhow::bail!("defined same subsidy threshold multiple times");
            }
        }

        Ok(SubsidyTiers(tiers))
    }
}

pub struct CowSubsidy {
    token: CowProtocolToken,
    vtoken: CowProtocolVirtualToken,
    subsidy_tiers: SubsidyTiers,
    balance_fetcher: Arc<dyn BalanceFetching>,
}

impl CowSubsidy {
    pub fn new(
        token: CowProtocolToken,
        vtoken: CowProtocolVirtualToken,
        subsidy_tiers: SubsidyTiers,
        balance_fetcher: Arc<dyn BalanceFetching>,
    ) -> Self {
        Self {
            token,
            vtoken,
            subsidy_tiers,
            balance_fetcher,
        }
    }

    async fn cow_subsidy_factor(&self, user: H160) -> Result<f64> {
        let queries = vec![
            Query {
                owner: user,
                token: self.token.address(),
                source: SellTokenSource::Erc20,
            },
            Query {
                owner: user,
                token: self.vtoken.address(),
                source: SellTokenSource::Erc20,
            },
        ];
        let balances = self
            .balance_fetcher
            .get_balances(&queries)
            .await
            .into_iter()
            .collect::<Result<Vec<U256>>>()?;
        let cow_balance = balances[0];
        let vcow_balance = balances[1];

        let combined = cow_balance.saturating_add(vcow_balance);
        let tier = self.subsidy_tiers.0.range(..=combined).rev().next();
        let factor = tier.map(|tier| *tier.1).unwrap_or(1.0);
        tracing::debug!(?user, ?cow_balance, ?vcow_balance, ?combined, ?factor);
        Ok(factor)
    }
}

#[async_trait::async_trait]
impl FeeSubsidizing for CowSubsidy {
    async fn subsidy(&self, parameters: SubsidyParameters) -> Result<Subsidy> {
        Ok(Subsidy {
            factor: self.cow_subsidy_factor(parameters.from).await?,
            ..Default::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{account_balances::Web3BalanceFetcher, ethrpc::Web3},
        hex_literal::hex,
    };

    #[tokio::test]
    #[ignore]
    async fn mainnet() {
        crate::tracing::initialize_reentrant("orderbook=debug");
        let transport = crate::ethrpc::create_env_test_transport();
        let web3 = Web3::new(transport);
        let token = CowProtocolToken::deployed(&web3).await.unwrap();
        let vtoken = CowProtocolVirtualToken::deployed(&web3).await.unwrap();
        let settlement = contracts::GPv2Settlement::deployed(&web3).await.unwrap();
        let vault_relayer = settlement.vault_relayer().call().await.unwrap();
        let balance_fetcher = Arc::new(Web3BalanceFetcher::new(
            web3.clone(),
            None,
            vault_relayer,
            settlement.address(),
        ));
        let subsidy = CowSubsidy::new(
            token,
            vtoken,
            SubsidyTiers([(U256::from_f64_lossy(1e18), 0.5)].into_iter().collect()),
            balance_fetcher,
        );
        //
        for user in [
            hex!("0000000000000000000000000000000000000000"),
            hex!("ca07eaa4253638d286cad71cbceec11803f2709a"),
            hex!("de1c59bc25d806ad9ddcbe246c4b5e5505645718"),
        ] {
            let user = H160(user);
            let result = subsidy.cow_subsidy_factor(user).await;
            println!("{user:?} {result:?}");
        }
    }
}
