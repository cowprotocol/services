//! An `eth_call` simulation-based balance reading implementation. This allows
//! balances and allowances to be fetched as well as transfers to be verified
//! from a node in a single round-trip, while accounting for pre-interactions.

use {
    super::{BalanceFetching, Query, TransferSimulationError},
    crate::account_balances::BalanceSimulator,
    alloy::primitives::U256,
    anyhow::Result,
    contracts::alloy::{BalancerV2Vault::BalancerV2Vault, ERC20::ERC20::ERC20Instance},
    ethrpc::{AlloyProvider, Web3, alloy::ProviderLabelingExt},
    futures::{FutureExt, StreamExt, future::BoxFuture, stream::BoxStream},
    model::order::SellTokenSource,
    std::sync::Arc,
    tracing::instrument,
};

pub struct Balances(Arc<Inner>);

impl Balances {
    pub fn new(web3: &Web3, simulator: BalanceSimulator) -> Self {
        // Note that the balances simulation **will fail** if the `vault`
        // address is not a contract and the `source` is set to one of
        // `SellTokenSource::{External, Internal}` (i.e. the Vault contract is
        // needed). This is because Solidity generates code to verify that
        // contracts exist at addresses that get called. This allows us to
        // properly check if the `source` is not supported for the deployment
        // work without additional code paths :tada:!
        let web3 = web3.labeled("balanceFetching");

        Self(Arc::new(Inner {
            provider: web3.provider,
            simulator,
        }))
    }
}

struct Inner {
    provider: AlloyProvider,
    simulator: BalanceSimulator,
}

impl Inner {
    async fn tradable_balance(&self, query: &Query) -> Result<U256> {
        if !query.interactions.is_empty() {
            return self
                .simulator
                .simulate(
                    query.owner,
                    query.token,
                    query.source,
                    &query.interactions,
                    None,
                    query.balance_override.clone(),
                )
                .await
                .map(|sim| {
                    if sim.can_transfer {
                        sim.effective_balance
                    } else {
                        U256::ZERO
                    }
                })
                .map_err(Into::into);
        }

        let token = ERC20Instance::new(query.token, &self.provider);
        let usable_balance = match query.source {
            SellTokenSource::Erc20 => {
                let balance = token.balanceOf(query.owner);
                let allowance = token.allowance(query.owner, self.simulator.vault_relayer());
                let (balance, allowance) = futures::try_join!(
                    balance.call().into_future(),
                    allowance.call().into_future()
                )?;
                std::cmp::min(balance, allowance)
            }
            SellTokenSource::External => {
                let vault = BalancerV2Vault::new(self.simulator.vault(), &self.provider);
                let balance = token.balanceOf(query.owner);
                let approved =
                    vault.hasApprovedRelayer(query.owner, self.simulator.vault_relayer());
                let allowance = token.allowance(query.owner, self.simulator.vault());
                let (balance, approved, allowance) = futures::try_join!(
                    balance.call().into_future(),
                    approved.call().into_future(),
                    allowance.call().into_future()
                )?;
                match approved {
                    true => std::cmp::min(balance, allowance),
                    false => alloy::primitives::U256::ZERO,
                }
            }
            SellTokenSource::Internal => {
                let vault = BalancerV2Vault::new(self.simulator.vault(), &self.provider);
                let balance = vault.getInternalBalance(query.owner, vec![query.token]);
                let approved =
                    vault.hasApprovedRelayer(query.owner, self.simulator.vault_relayer());
                let (balance, approved) = futures::try_join!(
                    balance.call().into_future(),
                    approved.call().into_future()
                )?;
                match approved {
                    true => balance[0], // internal approvals are always U256::MAX
                    false => alloy::primitives::U256::ZERO,
                }
            }
        };
        Ok(usable_balance)
    }
}

#[async_trait::async_trait]
impl BalanceFetching for Balances {
    #[instrument(skip_all)]
    fn get_balances(
        &self,
        queries: Vec<Query>,
    ) -> BoxStream<'_, BoxFuture<'static, (Query, anyhow::Result<U256>)>> {
        // TODO(nlordell): Use `Multicall` here to use fewer node round-trips
        let futs = queries.into_iter().map(move |query| {
            let inner = self.0.clone();
            async move {
                let res = inner.tradable_balance(&query).await;
                (query, res)
            }
            .boxed()
        });
        futures::stream::iter(futs).boxed()
    }

    async fn can_transfer(
        &self,
        query: &Query,
        amount: U256,
    ) -> Result<(), TransferSimulationError> {
        let simulation = self
            .0
            .simulator
            .simulate(
                query.owner,
                query.token,
                query.source,
                &query.interactions,
                Some(amount),
                query.balance_override.clone(),
            )
            .await
            .map_err(|err| TransferSimulationError::Other(err.into()))?;

        if simulation.token_balance < amount {
            return Err(TransferSimulationError::InsufficientBalance);
        }
        if simulation.allowance < amount {
            return Err(TransferSimulationError::InsufficientAllowance);
        }
        if !simulation.can_transfer {
            return Err(TransferSimulationError::TransferFailed);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::price_estimation::trade_verifier::balance_overrides::DummyOverrider,
        alloy::primitives::address,
        contracts::alloy::GPv2Settlement,
        ethrpc::Web3,
        model::order::SellTokenSource,
        std::sync::Arc,
    };

    #[ignore]
    #[tokio::test]
    async fn test_for_user() {
        let web3 = Web3::new_from_env();
        let settlement = GPv2Settlement::GPv2Settlement::new(
            alloy::primitives::address!("0x9008d19f58aabd9ed0d60971565aa8510560ab41"),
            web3.provider.clone(),
        );
        let balances = contracts::alloy::support::Balances::Instance::new(
            address!("3e8C6De9510e7ECad902D005DE3Ab52f35cF4f1b"),
            web3.provider.clone(),
        );
        let balances = Balances::new(
            &web3,
            BalanceSimulator::new(
                settlement,
                balances,
                address!("C92E8bdf79f0507f65a392b0ab4667716BFE0110"),
                Some(address!("BA12222222228d8Ba445958a75a0704d566BF2C8")),
                Arc::new(DummyOverrider),
            ),
        );

        let owner = address!("b0a4e99371dfb0734f002ae274933b4888f618ef");
        let token = address!("d909c5862cdb164adb949d92622082f0092efc3d");
        let amount = U256::from(50000000000000000000000_u128);
        let source = SellTokenSource::Erc20;

        balances
            .can_transfer(
                &Query {
                    owner,
                    token,
                    source,
                    interactions: vec![],
                    balance_override: None,
                },
                amount,
            )
            .await
            .unwrap();
        println!("{owner:?} can transfer {amount} {token:?}!");
    }
}
