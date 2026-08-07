//! An `eth_call` simulation-based balance reading implementation. This allows
//! balances and allowances to be fetched as well as transfers to be verified
//! from a node in a single round-trip, while accounting for pre-interactions.

use {
    super::{BalanceFetching, Query, TransferSimulationError},
    crate::BalanceSimulator,
    alloy_primitives::{Address, U256},
    alloy_provider::{CallItem, Provider},
    alloy_sol_types::SolCall,
    anyhow::{Context, Result, ensure},
    contracts::{ERC20, Multicall3},
    ethrpc::{Web3, alloy::ProviderLabelingExt},
    futures::future,
    itertools::Itertools,
    model::order::SellTokenSource,
};

/// How many queries to bundle into a single `Multicall3` call. Every query
/// contributes two sub-calls (`balanceOf` + `allowance`), so the limit is
/// really about staying well below the node's `eth_call` gas cap for tokens
/// with expensive accessors.
const MULTICALL_BATCH_SIZE: usize = 10;

pub struct Balances {
    web3: Web3,
    balance_simulator: BalanceSimulator,
    /// `Multicall3` on this chain, if we know of a deployment. Without one,
    /// balances are read one by one.
    multicall: Option<Address>,
}

impl Balances {
    pub fn new(web3: &Web3, balance_simulator: BalanceSimulator, chain_id: u64) -> Self {
        let web3 = web3.labeled("balanceFetching");

        let multicall = Multicall3::deployment_address(&chain_id);
        if multicall.is_none() {
            tracing::warn!(
                chain_id,
                "no Multicall3 deployment; reading balances one by one"
            );
        }

        Self {
            web3,
            balance_simulator,
            multicall,
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

    /// Reads the tradable balances of queries without pre-interactions, in the
    /// order they were given. All queries must have an ERC20 sell-token source.
    async fn tradable_balances_no_simulation(&self, queries: &[&Query]) -> Vec<Result<U256>> {
        let Some(multicall) = self.multicall else {
            return self.tradable_balances_simple(queries).await;
        };

        let chunks = queries
            .chunks(MULTICALL_BATCH_SIZE)
            .map(|chunk| async move {
                match self.tradable_balances_multicall(chunk, multicall).await {
                    Ok(balances) => balances,
                    // A whole batch failing is not something we can attribute to any
                    // single query (most likely the node hit its `eth_call` gas cap),
                    // so retry the chunk without batching rather than failing it.
                    Err(err) => {
                        tracing::warn!(?err, "batched balance call failed, retrying individually");
                        self.tradable_balances_simple(chunk).await
                    }
                }
            });

        future::join_all(chunks)
            .await
            .into_iter()
            .flatten()
            .collect()
    }

    /// Reads the balances and allowances of a chunk of queries with a single
    /// `Multicall3` call.
    async fn tradable_balances_multicall(
        &self,
        queries: &[&Query],
        multicall_address: Address,
    ) -> Result<Vec<Result<U256>>> {
        // A dynamic multicall decodes every result with the same decoder. That
        // works for both of our calls because `balanceOf` and `allowance` return
        // a single `uint256`.
        let mut multicall = self
            .web3
            .provider
            .multicall()
            .address(multicall_address)
            .dynamic::<ERC20::ERC20::balanceOfCall>();
        for query in queries {
            // A single misbehaving token must not fail the whole batch.
            let call =
                |input: Vec<u8>| CallItem::new(query.token, input.into()).with_failure_allowed();
            multicall = multicall
                .add_call_dynamic(call(
                    ERC20::ERC20::balanceOfCall {
                        account: query.owner,
                    }
                    .abi_encode(),
                ))
                .add_call_dynamic(call(
                    ERC20::ERC20::allowanceCall {
                        owner: query.owner,
                        spender: self.vault_relayer(),
                    }
                    .abi_encode(),
                ));
        }

        let results = multicall.aggregate3().await?;
        ensure!(
            results.len() == queries.len() * 2,
            "expected {} multicall results, got {}",
            queries.len() * 2,
            results.len()
        );

        Ok(results
            .into_iter()
            .tuples()
            .map(|(balance, allowance)| {
                let balance = balance.context("could not read balance")?;
                let allowance = allowance.context("could not read allowance")?;
                Ok(std::cmp::min(balance, allowance))
            })
            .collect())
    }

    /// Fallback for chains and nodes that cannot serve batched calls.
    async fn tradable_balances_simple(&self, queries: &[&Query]) -> Vec<Result<U256>> {
        future::join_all(queries.iter().map(|query| async move {
            let token = ERC20::Instance::new(query.token, self.web3.provider.clone());
            let balance = token.balanceOf(query.owner);
            let allowance = token.allowance(query.owner, self.vault_relayer());
            let (balance, allowance) =
                futures::try_join!(balance.call().into_future(), allowance.call().into_future())?;
            Ok(std::cmp::min(balance, allowance))
        }))
        .await
    }
}

#[async_trait::async_trait]
impl BalanceFetching for Balances {
    async fn get_balances(&self, queries: &[Query]) -> Vec<Result<U256>> {
        // Queries with pre-interactions have to be simulated from the settlement
        // contract's context one by one. The rest are plain `balanceOf` and
        // `allowance` reads which we can batch into very few node round-trips.
        let (simple, simulated): (Vec<_>, Vec<_>) = queries
            .iter()
            .enumerate()
            .partition(|(_, q)| q.source == SellTokenSource::Erc20 && q.interactions.is_empty());

        let simple_queries: Vec<_> = simple.iter().map(|(_, query)| *query).collect();
        let (simple_balances, simulated_balances) = futures::join!(
            self.tradable_balances_no_simulation(&simple_queries),
            future::join_all(
                simulated
                    .iter()
                    .map(|(_, query)| self.tradable_balance_simulated(query))
            ),
        );

        let mut results: Vec<_> = simple
            .into_iter()
            .map(|(index, _)| index)
            .zip(simple_balances)
            .chain(
                simulated
                    .into_iter()
                    .map(|(index, _)| index)
                    .zip(simulated_balances),
            )
            .collect();
        results.sort_by_key(|(index, _)| *index);
        results.into_iter().map(|(_, balance)| balance).collect()
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
        let token = ERC20::Instance::new(token, self.web3.provider.clone());
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
            &web3,
            BalanceSimulator::new(
                settlement,
                balances,
                address!("C92E8bdf79f0507f65a392b0ab4667716BFE0110"),
                Arc::new(DummyStateOverrider),
            ),
            1,
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
