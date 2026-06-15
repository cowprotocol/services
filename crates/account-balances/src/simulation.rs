//! An `eth_call` simulation-based balance reading implementation. This allows
//! balances and allowances to be fetched as well as transfers to be verified
//! from a node in a single round-trip, while accounting for pre-interactions.

use {
    super::{BalanceFetching, Query, TransferSimulationError},
    crate::{BalanceSimulator, BlockNumber, SimulateParams},
    alloy_primitives::{Address, U256},
    alloy_rpc_types::BlockId,
    anyhow::Result,
    contracts::{BalancerV2Vault::BalancerV2Vault, ERC20},
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

    fn block_id_from_number(block_number: Option<BlockNumber>) -> BlockId {
        block_number
            .map(BlockId::number)
            .unwrap_or_else(BlockId::latest)
    }

    async fn tradable_balance_simulated(
        &self,
        query: &Query,
        block_number: Option<BlockNumber>,
    ) -> Result<U256> {
        let simulation = self
            .balance_simulator
            .simulate(
                query.owner,
                query.token,
                query.source,
                &query.interactions,
                SimulateParams::new(None, query.balance_override.clone(), block_number),
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
        block_number: Option<BlockNumber>,
    ) -> Result<U256> {
        let block_id = Self::block_id_from_number(block_number);

        let usable_balance = match query.source {
            SellTokenSource::Erc20 => {
                let balance_call = token.balanceOf(query.owner).block(block_id);
                let allowance_call = token
                    .allowance(query.owner, self.vault_relayer())
                    .block(block_id);
                let (balance, allowance) = futures::try_join!(
                    balance_call.call().into_future(),
                    allowance_call.call().into_future()
                )?;
                std::cmp::min(balance, allowance)
            }
            SellTokenSource::External => {
                let vault = BalancerV2Vault::new(self.vault(), &self.web3.provider);
                let balance_call = token.balanceOf(query.owner).block(block_id);
                let approved_call = vault
                    .hasApprovedRelayer(query.owner, self.vault_relayer())
                    .block(block_id);
                let allowance_call = token.allowance(query.owner, self.vault()).block(block_id);
                let (balance, approved, allowance) = futures::try_join!(
                    balance_call.call().into_future(),
                    approved_call.call().into_future(),
                    allowance_call.call().into_future()
                )?;
                match approved {
                    true => std::cmp::min(balance, allowance),
                    false => U256::ZERO,
                }
            }
            SellTokenSource::Internal => {
                let vault = BalancerV2Vault::new(self.vault(), &self.web3.provider);
                let tokens = vec![query.token];
                let balance_call = vault
                    .getInternalBalance(query.owner, tokens)
                    .block(block_id);
                let approved_call = vault
                    .hasApprovedRelayer(query.owner, self.vault_relayer())
                    .block(block_id);
                let (balance, approved) = futures::try_join!(
                    balance_call.call().into_future(),
                    approved_call.call().into_future()
                )?;
                match approved {
                    true => balance[0],
                    false => U256::ZERO,
                }
            }
        };
        Ok(usable_balance)
    }
}

#[async_trait::async_trait]
impl BalanceFetching for Balances {
    #[instrument(skip_all)]
    async fn get_balances(
        &self,
        queries: &[Query],
        block_number: Option<BlockNumber>,
    ) -> Vec<Result<U256>> {
        let futures = queries
            .iter()
            .map(|query| async {
                if query.interactions.is_empty() {
                    let token = ERC20::Instance::new(query.token, self.web3.provider.clone());
                    self.tradable_balance_simple(query, &token, block_number)
                        .await
                } else {
                    self.tradable_balance_simulated(query, block_number).await
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
                SimulateParams::new(Some(amount), query.balance_override.clone(), None),
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
            &web3,
            BalanceSimulator::new(
                settlement,
                balances,
                address!("C92E8bdf79f0507f65a392b0ab4667716BFE0110"),
                Some(address!("BA12222222228d8Ba445958a75a0704d566BF2C8")),
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
