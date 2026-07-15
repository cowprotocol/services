//! An `eth_call` simulation-based balance reading implementation. This allows
//! balances and allowances to be fetched as well as transfers to be verified
//! from a node in a single round-trip, while accounting for pre-interactions.

use {
    super::{BalanceFetching, Query, TransferSimulationError},
    crate::BalanceSimulator,
    alloy_primitives::{Address, U256},
    anyhow::Result,
    contracts::ERC20,
    ethrpc::{AlloyProvider, alloy::ProviderLabelingExt},
    futures::future,
    model::order::SellTokenSource,
    tracing::instrument,
};

pub struct Balances {
    provider: AlloyProvider,
    balance_simulator: BalanceSimulator,
}

impl Balances {
    pub fn new(provider: &AlloyProvider, balance_simulator: BalanceSimulator) -> Self {
        let provider = provider.labeled("balanceFetching");

        Self {
            provider,
            balance_simulator,
        }
    }

    fn vault_relayer(&self) -> Address {
        self.balance_simulator.vault_relayer
    }

    async fn tradable_balance_simulated(&self, query: &Query) -> Result<U256> {
        // Only ERC20 sell-token balances are supported; other sources are deprecated
        // and rejected at order creation.
        if query.source != SellTokenSource::Erc20 {
            anyhow::bail!("unsupported sell token source: {:?}", query.source);
        }
        let simulation = self
            .balance_simulator
            .simulate(
                query.owner,
                query.token,
                query.source,
                &query.interactions,
                None,
                query.balance_override.clone(),
            )
            .await?;
        Ok(if simulation.can_transfer {
            simulation.effective_balance
        } else {
            U256::ZERO
        })
    }

    async fn tradable_balance_simple(
        &self,
        query: &Query,
        token: &ERC20::Instance,
    ) -> Result<U256> {
        // Only ERC20 sell-token balances are supported. Other sources are deprecated
        // and rejected at order creation.
        if query.source != SellTokenSource::Erc20 {
            anyhow::bail!("unsupported sell token source: {:?}", query.source);
        }
        let balance = token.balanceOf(query.owner);
        let allowance = token.allowance(query.owner, self.vault_relayer());
        let (balance, allowance) =
            futures::try_join!(balance.call().into_future(), allowance.call().into_future())?;
        Ok(std::cmp::min(balance, allowance))
    }
}

#[async_trait::async_trait]
impl BalanceFetching for Balances {
    #[instrument(skip_all)]
    async fn get_balances(&self, queries: &[Query]) -> Vec<Result<U256>> {
        // TODO(nlordell): Use `Multicall` here to use fewer node round-trips
        let futures = queries
            .iter()
            .map(|query| async {
                if query.interactions.is_empty() {
                    let token = ERC20::Instance::new(query.token, self.provider.clone());
                    self.tradable_balance_simple(query, &token).await
                } else {
                    self.tradable_balance_simulated(query).await
                }
            })
            .collect::<Vec<_>>();

        future::join_all(futures).await
    }

    async fn can_transfer(
        &self,
        query: &Query,
        amount: U256,
    ) -> Result<(), TransferSimulationError> {
        let simulation = self
            .balance_simulator
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
            return Err(TransferSimulationError::TransferFailed(
                simulation.transfer_revert_reason,
            ));
        }

        Ok(())
    }

    #[instrument(skip(self))]
    async fn allowance(
        &self,
        owner: Address,
        token: Address,
        source: SellTokenSource,
    ) -> Result<U256> {
        // Only ERC20 sell-token balances are supported; other sources are deprecated
        // and rejected at order creation.
        if source != SellTokenSource::Erc20 {
            anyhow::bail!("unsupported sell token source: {:?}", source);
        }
        let token = ERC20::Instance::new(token, self.provider.clone());
        Ok(token.allowance(owner, self.vault_relayer()).call().await?)
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy_primitives::address,
        balance_overrides::DummyStateOverrider,
        contracts::GPv2Settlement,
        ethrpc::Web3,
        model::order::SellTokenSource,
        std::sync::Arc,
    };

    #[ignore]
    #[tokio::test]
    async fn test_for_user() {
        let web3 = Web3::new_from_env();
        let settlement = GPv2Settlement::GPv2Settlement::new(
            address!("0x9008d19f58aabd9ed0d60971565aa8510560ab41"),
            web3.provider.clone(),
        );
        let balances = contracts::support::Balances::Instance::new(
            address!("3e8C6De9510e7ECad902D005DE3Ab52f35cF4f1b"),
            web3.provider.clone(),
        );
        let balances = Balances::new(
            &web3.provider,
            BalanceSimulator::new(
                settlement,
                balances,
                address!("C92E8bdf79f0507f65a392b0ab4667716BFE0110"),
                Arc::new(DummyStateOverrider),
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
