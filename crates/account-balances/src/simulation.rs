//! An `eth_call` simulation-based balance reading implementation. This allows
//! balances and allowances to be fetched as well as transfers to be verified
//! from a node in a single round-trip, while accounting for pre-interactions.

use {
    super::{BalanceFetching, Query, TransferSimulationError},
    crate::BalanceSimulator,
    alloy_primitives::{Address, U256},
    alloy_provider::{CallItem, MulticallError, Provider},
    alloy_sol_types::SolCall,
    anyhow::{Context, Result, anyhow, ensure},
    contracts::ERC20,
    ethrpc::{Web3, alloy::ProviderLabelingExt},
    futures::future,
    itertools::Itertools,
    model::order::SellTokenSource,
};

/// How many queries go into a single `Multicall3` call. Every query adds two
/// sub-calls (`balanceOf` + `allowance`), so the bound is really about staying
/// below the node's `eth_call` gas cap for tokens with expensive accessors.
const MULTICALL_BATCH_SIZE: usize = 20;

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

    async fn tradable_balance_simulated(&self, query: &Query) -> Result<U256> {
        ensure_erc20(query.source)?;
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
    /// order they were given. Every query must have an ERC20 sell-token source.
    async fn tradable_balances_simple(&self, queries: &[&Query]) -> Vec<Result<U256>> {
        let chunks = queries
            .chunks(MULTICALL_BATCH_SIZE)
            .map(|chunk| async move {
                match self.tradable_balances_batched(chunk).await {
                    Ok(balances) => balances,
                    // The node refusing a batch it did answer is not attributable to
                    // any single query, and splitting the chunk up gets back under
                    // whatever limit it hit. The individual reads are the single
                    // retry; they are never retried again.
                    Err(BatchError::Refused(err)) => {
                        tracing::warn!(
                            ?err,
                            queries = chunk.len(),
                            "node refused batched balance call, retrying unbatched"
                        );
                        self.tradable_balances_individually(chunk).await
                    }
                    // Retrying a chunk that never got an answer would only pile more
                    // load onto a node that is already failing, so report the failure
                    // per query the way an unbatched read would have.
                    Err(BatchError::Failed(err)) => {
                        tracing::warn!(?err, queries = chunk.len(), "batched balance call failed");
                        let err = format!("{err:#}");
                        chunk
                            .iter()
                            .map(|_| Err(anyhow!("batched balance call failed: {err}")))
                            .collect()
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
    async fn tradable_balances_batched(
        &self,
        queries: &[&Query],
    ) -> Result<Vec<Result<U256>>, BatchError> {
        // A dynamic multicall decodes every result with the same decoder, which
        // works here because `balanceOf` and `allowance` both return a single
        // `uint256`.
        let mut multicall = self
            .web3
            .provider
            .multicall()
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

        let results = multicall.aggregate3().await.map_err(BatchError::from)?;
        if results.len() != queries.len() * 2 {
            return Err(BatchError::Failed(anyhow!(
                "expected {} multicall results, got {}",
                queries.len() * 2,
                results.len()
            )));
        }

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

    /// Fallback for nodes that cannot serve a batched call.
    async fn tradable_balances_individually(&self, queries: &[&Query]) -> Vec<Result<U256>> {
        future::join_all(queries.iter().map(|query| async move {
            let token = ERC20::Instance::new(query.token, self.web3.provider.clone());
            let (balance, allowance) = self
                .web3
                .provider
                .multicall()
                .add(token.balanceOf(query.owner))
                .add(token.allowance(query.owner, self.vault_relayer()))
                .aggregate()
                .await?;
            Ok(std::cmp::min(balance, allowance))
        }))
        .await
    }
}

/// Why a whole `Multicall3` call did not produce results.
enum BatchError {
    /// The node answered by refusing the call, most likely because the batch
    /// exceeded a limit of its own such as the `eth_call` gas cap.
    Refused(anyhow::Error),
    /// The call produced no usable answer at all.
    Failed(anyhow::Error),
}

impl From<MulticallError> for BatchError {
    fn from(err: MulticallError) -> Self {
        // Only an error response proves the node executed the batch and turned it
        // down over its size; every other failure says nothing about the batch, so
        // splitting it up cannot be expected to help.
        match &err {
            MulticallError::TransportError(rpc) if rpc.is_error_resp() => Self::Refused(err.into()),
            _ => Self::Failed(err.into()),
        }
    }
}

/// Only ERC20 sell-token balances are supported; the other sources are
/// deprecated and rejected at order creation.
fn ensure_erc20(source: SellTokenSource) -> Result<()> {
    ensure!(
        source == SellTokenSource::Erc20,
        "unsupported sell token source: {source:?}"
    );
    Ok(())
}

#[async_trait::async_trait]
impl BalanceFetching for Balances {
    async fn get_balances(&self, queries: &[Query]) -> Vec<Result<U256>> {
        // Queries with pre-interactions have to be simulated from the settlement
        // contract's context one by one. The rest are plain `balanceOf` and
        // `allowance` reads, which batch into far fewer node round-trips.
        let mut unsupported = Vec::new();
        let mut simple = Vec::new();
        let mut simulated = Vec::new();
        for (index, query) in queries.iter().enumerate() {
            match ensure_erc20(query.source) {
                Err(err) => unsupported.push((index, Err(err))),
                Ok(()) if query.interactions.is_empty() => simple.push((index, query)),
                Ok(()) => simulated.push((index, query)),
            }
        }

        let simple_queries: Vec<_> = simple.iter().map(|(_, query)| *query).collect();
        let (simple_balances, simulated_balances) = futures::join!(
            self.tradable_balances_simple(&simple_queries),
            future::join_all(
                simulated
                    .iter()
                    .map(|(_, query)| self.tradable_balance_simulated(query))
            ),
        );

        unsupported
            .into_iter()
            .chain(
                simple
                    .into_iter()
                    .map(|(index, _)| index)
                    .zip(simple_balances),
            )
            .chain(
                simulated
                    .into_iter()
                    .map(|(index, _)| index)
                    .zip(simulated_balances),
            )
            .sorted_by_key(|(idx, _)| *idx)
            .map(|(_, balance)| balance)
            .collect()
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
        ensure_erc20(source)?;
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

    fn mainnet_balances(web3: &Web3) -> Balances {
        let settlement = GPv2Settlement::GPv2Settlement::new(
            address!("0x9008d19f58aabd9ed0d60971565aa8510560ab41"),
            web3.provider.clone(),
        );
        let balances = contracts::support::Balances::Instance::new(
            address!("3e8C6De9510e7ECad902D005DE3Ab52f35cF4f1b"),
            web3.provider.clone(),
        );
        Balances::new(
            web3,
            BalanceSimulator::new(
                settlement,
                balances,
                address!("C92E8bdf79f0507f65a392b0ab4667716BFE0110"),
                Arc::new(DummyStateOverrider),
            ),
        )
    }

    #[ignore]
    #[tokio::test]
    async fn test_for_user() {
        let web3 = Web3::new_from_env();
        let balances = mainnet_balances(&web3);

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

    /// Batching must not change any result compared to reading the balances one
    /// by one, including for tokens that make the reads fail.
    #[ignore]
    #[tokio::test]
    async fn test_batching_matches_individual_calls() {
        let web3 = Web3::new_from_env();
        let balances = mainnet_balances(&web3);

        let query = |owner, token| Query {
            owner,
            token,
            source: SellTokenSource::Erc20,
            interactions: vec![],
            balance_override: None,
        };

        let weth = address!("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
        let usdc = address!("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
        // An address without any code, so both paths have to report a failure.
        let not_a_token = address!("0x1111111111111111111111111111111111111111");

        let mut queries = vec![
            query(address!("0x28C6c06298d514Db089934071355E5743bf21d60"), usdc),
            query(address!("0x28C6c06298d514Db089934071355E5743bf21d60"), weth),
            query(
                address!("0x28C6c06298d514Db089934071355E5743bf21d60"),
                not_a_token,
            ),
        ];
        // Push past `MULTICALL_BATCH_SIZE` so that chunking is exercised too.
        for i in 0..MULTICALL_BATCH_SIZE {
            queries.push(query(Address::repeat_byte(i as u8), weth));
        }

        let batched = balances.get_balances(&queries).await;
        let individual = balances
            .tradable_balances_individually(&queries.iter().collect::<Vec<_>>())
            .await;

        assert_eq!(batched.len(), queries.len());
        for (index, (batched, individual)) in batched.iter().zip(&individual).enumerate() {
            match (batched, individual) {
                (Ok(batched), Ok(individual)) => assert_eq!(batched, individual, "query {index}"),
                (Err(_), Err(_)) => (),
                _ => panic!("query {index} disagrees: {batched:?} vs {individual:?}"),
            }
        }
        // The non-token must actually have failed, otherwise the assertion above
        // would pass without ever comparing an error.
        assert!(batched[2].is_err());
    }
}
