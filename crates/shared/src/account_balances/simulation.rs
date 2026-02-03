//! An `eth_call` simulation-based balance reading implementation. This allows
//! balances and allowances to be fetched as well as transfers to be verified
//! from a node in a single round-trip, while accounting for pre-interactions.

use {
    super::{BalanceFetching, Query, TransferSimulationError},
    crate::account_balances::BalanceSimulator,
    alloy::primitives::{Address, U256},
    anyhow::Result,
    contracts::alloy::{BalancerV2Vault::BalancerV2Vault, ERC20},
    ethrpc::{Web3, alloy::ProviderLabelingExt},
    futures::future,
    model::order::SellTokenSource,
    tracing::instrument,
};

pub struct Balances {
    web3: Web3,
    balance_simulator: BalanceSimulator,
}

impl Balances {
    pub fn new(web3: &Web3, balance_simulator: BalanceSimulator) -> Self {
        // Note that the balances simulation **will fail** if the `vault`
        // address is not a contract and the `source` is set to one of
        // `SellTokenSource::{External, Internal}` (i.e. the Vault contract is
        // needed). This is because Solidity generates code to verify that
        // contracts exist at addresses that get called. This allows us to
        // properly check if the `source` is not supported for the deployment
        // work without additional code paths :tada:!
        let web3 = web3.labeled("balanceFetching");

        Self {
            web3,
            balance_simulator,
        }
    }

    fn vault_relayer(&self) -> Address {
        self.balance_simulator.vault_relayer
    }

    fn vault(&self) -> Address {
        self.balance_simulator.vault
    }

    async fn tradable_balance_simulated(&self, query: &Query) -> Result<U256> {
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
        let usable_balance = match query.source {
            SellTokenSource::Erc20 => {
                let balance = token.balanceOf(query.owner);
                let allowance = token.allowance(query.owner, self.vault_relayer());
                let (balance, allowance) = futures::try_join!(
                    balance.call().into_future(),
                    allowance.call().into_future()
                )?;
                std::cmp::min(balance, allowance)
            }
            SellTokenSource::External => {
                let vault = BalancerV2Vault::new(self.vault(), &self.web3.alloy);
                let balance = token.balanceOf(query.owner);
                let approved = vault.hasApprovedRelayer(query.owner, self.vault_relayer());
                let allowance = token.allowance(query.owner, self.vault());
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
                let vault = BalancerV2Vault::new(self.vault(), &self.web3.alloy);
                let balance = vault.getInternalBalance(query.owner, vec![query.token]);
                let approved = vault.hasApprovedRelayer(query.owner, self.vault_relayer());
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
    async fn get_balances(&self, queries: &[Query]) -> Vec<Result<U256>> {
        // TODO(nlordell): Use `Multicall` here to use fewer node round-trips
        let futures = queries
            .iter()
            .map(|query| async {
                if query.interactions.is_empty() {
                    let token = ERC20::Instance::new(query.token, self.web3.alloy.clone());
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
            web3.alloy.clone(),
        );
        let balances = contracts::alloy::support::Balances::Instance::new(
            address!("3e8C6De9510e7ECad902D005DE3Ab52f35cF4f1b"),
            web3.alloy.clone(),
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
