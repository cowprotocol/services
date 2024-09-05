//! An `eth_call` simulation-based balance reading implementation. This allows
//! balances and allowances to be fetched as well as transfers to be verified
//! from a node in a single round-trip, while accounting for pre-interactions.

use {
    super::{BalanceFetching, Query, TransferSimulationError},
    anyhow::{anyhow, Result},
    contracts::{erc20::Contract, BalancerV2Vault},
    ethcontract::{Bytes, H160, U256},
    ethrpc::Web3,
    futures::future,
    model::order::SellTokenSource,
    std::collections::HashMap,
};

pub struct Balances {
    balances: contracts::support::Balances,
    web3: Web3,
    settlement: H160,
    vault_relayer: H160,
    vault: H160,
}

impl Balances {
    pub fn new(web3: &Web3, settlement: H160, vault_relayer: H160, vault: Option<H160>) -> Self {
        // Note that the balances simulation **will fail** if the `vault`
        // address is not a contract and the `source` is set to one of
        // `SellTokenSource::{External, Internal}` (i.e. the Vault contract is
        // needed). This is because Solidity generates code to verify that
        // contracts exist at addresses that get called. This allows us to
        // properly check if the `source` is not supported for the deployment
        // work without additional code paths :tada:!
        let vault = vault.unwrap_or_default();
        let web3 = ethrpc::instrumented::instrument_with_label(web3, "balanceFetching".into());
        let balances = contracts::support::Balances::at(&web3, settlement);

        Self {
            web3,
            balances,
            settlement,
            vault_relayer,
            vault,
        }
    }

    async fn simulate(&self, query: &Query, amount: Option<U256>) -> Result<Simulation> {
        // We simulate the balances from the Settlement contract's context. This
        // allows us to check:
        // 1. How the pre-interactions would behave as part of the settlement
        // 2. Simulate the actual VaultRelayer transfers that would happen as part of a
        //    settlement
        //
        // This allows us to end up with very accurate balance simulations.
        let (token_balance, allowance, effective_balance, can_transfer) =
            contracts::storage_accessible::simulate(
                contracts::bytecode!(contracts::support::Balances),
                self.balances.methods().balance(
                    (self.settlement, self.vault_relayer, self.vault),
                    query.owner,
                    query.token,
                    amount.unwrap_or_default(),
                    Bytes(query.source.as_bytes()),
                    query
                        .interactions
                        .iter()
                        .map(|i| (i.target, i.value, Bytes(i.call_data.clone())))
                        .collect(),
                ),
            )
            .await?;

        let simulation = Simulation {
            token_balance,
            allowance,
            effective_balance,
            can_transfer,
        };

        tracing::trace!(?query, ?amount, ?simulation, "simulated balances");
        Ok(simulation)
    }

    async fn tradable_balance_simulated(&self, query: &Query) -> Result<U256> {
        let simulation = self.simulate(query, None).await?;
        Ok(if simulation.can_transfer {
            simulation.effective_balance
        } else {
            U256::zero()
        })
    }

    async fn tradable_balance_simple(&self, query: &Query, token: &Contract) -> Result<U256> {
        let usable_balance = match query.source {
            SellTokenSource::Erc20 => {
                let balance = token.balance_of(query.owner).call();
                let allowance = token.allowance(query.owner, self.vault_relayer).call();
                let (balance, allowance) = futures::try_join!(balance, allowance)?;
                std::cmp::min(balance, allowance)
            }
            SellTokenSource::External => {
                let vault = BalancerV2Vault::at(&self.web3, self.vault);
                let balance = token.balance_of(query.owner).call();
                let approved = vault
                    .methods()
                    .has_approved_relayer(query.owner, self.vault_relayer)
                    .call();
                let allowance = token.allowance(query.owner, self.vault).call();
                let (balance, approved, allowance) =
                    futures::try_join!(balance, approved, allowance)?;
                match approved {
                    true => std::cmp::min(balance, allowance),
                    false => 0.into(),
                }
            }
            SellTokenSource::Internal => {
                let vault = BalancerV2Vault::at(&self.web3, self.vault);
                let balance = vault
                    .methods()
                    .get_internal_balance(query.owner, vec![query.token])
                    .call();
                let approved = vault
                    .methods()
                    .has_approved_relayer(query.owner, self.vault_relayer)
                    .call();
                let (balance, approved) = futures::try_join!(balance, approved)?;
                match approved {
                    true => balance[0], // internal approvals are always U256::MAX
                    false => 0.into(),
                }
            }
        };
        Ok(usable_balance)
    }
}

#[derive(Debug)]
struct Simulation {
    token_balance: U256,
    allowance: U256,
    effective_balance: U256,
    can_transfer: bool,
}

#[async_trait::async_trait]
impl BalanceFetching for Balances {
    async fn get_balances(&self, queries: &[Query]) -> Vec<Result<U256>> {
        let mut tokens: HashMap<_, _> = Default::default();
        for query in queries {
            if query.interactions.is_empty() {
                tokens
                    .entry(query.token)
                    .or_insert_with(|| contracts::ERC20::at(&self.web3, query.token));
            }
        }

        // TODO(nlordell): Use `Multicall` here to use fewer node round-trips
        let futures = queries
            .iter()
            .map(|query| async {
                if query.interactions.is_empty() {
                    let token = tokens
                        .get(&query.token)
                        .ok_or(anyhow!(format!("missing token {} contract", query.token)))?;
                    self.tradable_balance_simple(query, token).await
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
        let simulation = self.simulate(query, Some(amount)).await?;

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
        crate::ethrpc::{self, Web3},
        model::order::SellTokenSource,
    };

    #[ignore]
    #[tokio::test]
    async fn test_for_user() {
        let balances = Balances::new(
            &Web3::new(ethrpc::create_env_test_transport()),
            addr!("9008d19f58aabd9ed0d60971565aa8510560ab41"),
            addr!("C92E8bdf79f0507f65a392b0ab4667716BFE0110"),
            Some(addr!("BA12222222228d8Ba445958a75a0704d566BF2C8")),
        );

        let owner = addr!("b0a4e99371dfb0734f002ae274933b4888f618ef");
        let token = addr!("d909c5862cdb164adb949d92622082f0092efc3d");
        let amount = 50000000000000000000000_u128.into();
        let source = SellTokenSource::Erc20;

        balances
            .can_transfer(
                &Query {
                    owner,
                    token,
                    source,
                    interactions: vec![],
                },
                amount,
            )
            .await
            .unwrap();
        println!("{owner:?} can transfer {amount} {token:?}!");
    }
}
