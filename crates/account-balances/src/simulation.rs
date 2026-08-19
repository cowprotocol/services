//! An `eth_call` simulation-based balance reading implementation. This allows
//! balances and allowances to be fetched as well as transfers to be verified
//! from a node in a single round-trip, while accounting for pre-interactions.

use {
    super::{BalanceFetching, Query, TransferSimulationError},
    crate::BalanceSimulator,
    alloy_primitives::{Address, U256},
    alloy_provider::{CallItem, Provider},
    alloy_rpc_types::BlockId,
    alloy_sol_types::SolCall,
    anyhow::{Context, Result, ensure},
    contracts::ERC20,
    ethrpc::{Web3, alloy::ProviderLabelingExt},
    futures::{FutureExt, StreamExt, future},
    itertools::Itertools,
    model::order::SellTokenSource,
};

/// How many queries to bundle into a single `Multicall3` call. Every query
/// contributes two sub-calls (`balanceOf` + `allowance`), so the limit is
/// really about staying well below the node's `eth_call` gas cap for tokens
/// with expensive accessors.
const MULTICALL_BATCH_SIZE: usize = 50;

/// How many `Multicall3` calls to keep in flight at once. `aggregate3` goes
/// out as individual `eth_call`s that bypass the RPC batching layer, so
/// unbounded concurrency would flood the node.
const MAX_CONCURRENT_MULTICALLS: usize = 10;

pub struct Balances {
    web3: Web3,
    balance_simulator: BalanceSimulator,
    multicall_batch_size: usize,
    /// Block the balance reads are pinned to. `None` reads the latest state,
    /// which is what production wants.
    block: Option<BlockId>,
}

impl Balances {
    pub fn new(web3: &Web3, balance_simulator: BalanceSimulator) -> Self {
        let web3 = web3.labeled("balanceFetching");

        Self {
            web3,
            balance_simulator,
            multicall_batch_size: MULTICALL_BATCH_SIZE,
            block: None,
        }
    }

    /// Overrides how many queries are bundled into a single `Multicall3` call.
    /// `0` reads every balance individually. Exists so that the batch size can
    /// be tuned against a real node.
    pub fn with_multicall_batch_size(mut self, size: usize) -> Self {
        self.multicall_batch_size = size;
        self
    }

    /// Pins the balance reads to a fixed block, so that repeated reads return
    /// the same values instead of following the chain. Only affects the direct
    /// `balanceOf`/`allowance` reads, not the pre-interaction simulation.
    pub fn with_block(mut self, block: BlockId) -> Self {
        self.block = Some(block);
        self
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
    /// order they were given. All queries must have an ERC20 sell-token source.
    async fn tradable_balances_simple(&self, queries: &[&Query]) -> Vec<Result<U256>> {
        if self.multicall_batch_size == 0 {
            return self.tradable_balances_individually(queries).await;
        }

        // Boxing and eagerly collecting works around rustc's higher-ranked
        // lifetime inference bug when async blocks capturing references meet
        // `buffered`.
        tracing::debug!(
            queries = queries.len(),
            multicall_batch_size = self.multicall_batch_size,
            chunks = queries.len().div_ceil(self.multicall_batch_size),
            "reading balances through Multicall3"
        );

        let chunks: Vec<_> = queries
            .chunks(self.multicall_batch_size)
            .map(|chunk| {
                async move {
                    match self.tradable_balances_batched(chunk).await {
                        Ok(balances) => balances,
                        // A whole batch failing is not something we can attribute to
                        // any single query (most likely the node hit its `eth_call`
                        // gas cap), so retry the chunk without batching rather than
                        // failing it.
                        Err(err) => {
                            tracing::warn!(
                                ?err,
                                "batched balance call failed, retrying individually"
                            );
                            self.tradable_balances_individually(chunk).await
                        }
                    }
                }
                .boxed()
            })
            .collect();

        futures::stream::iter(chunks)
            .buffered(MAX_CONCURRENT_MULTICALLS)
            .concat()
            .await
    }

    /// Reads the balances and allowances of a chunk of queries with a single
    /// `Multicall3` call.
    async fn tradable_balances_batched(&self, queries: &[&Query]) -> Result<Vec<Result<U256>>> {
        // A dynamic multicall decodes every result with the same decoder. That
        // works for both of our calls because `balanceOf` and `allowance` return
        // a single `uint256`.
        let mut multicall = self
            .web3
            .provider
            .multicall()
            .dynamic::<ERC20::ERC20::balanceOfCall>();
        if let Some(block) = self.block {
            multicall = multicall.block(block);
        }
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

        // One `aggregate3`, so one `eth_call` — whether that `eth_call` then
        // shares an HTTP request with the other chunks' is up to the `ethrpc`
        // batching layer, which counts its own packets.
        tracing::debug!(
            queries = queries.len(),
            sub_calls = queries.len() * 2,
            "dispatching Multicall3 aggregate3"
        );
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
    async fn tradable_balances_individually(&self, queries: &[&Query]) -> Vec<Result<U256>> {
        future::join_all(queries.iter().map(|query| async move {
            let token = ERC20::Instance::new(query.token, self.web3.provider.clone());
            let mut balance = token.balanceOf(query.owner);
            let mut allowance = token.allowance(query.owner, self.vault_relayer());
            if let Some(block) = self.block {
                balance = balance.block(block);
                allowance = allowance.block(block);
            }
            let (balance, allowance) =
                futures::try_join!(balance.call().into_future(), allowance.call().into_future())?;
            Ok(std::cmp::min(balance, allowance))
        }))
        .await
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
        // `allowance` reads which we can batch into very few node round-trips.
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

        let mut results: Vec<_> = unsupported
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
}
