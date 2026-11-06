//! An `eth_call` simulation-based balance reading implementation. This allows
//! balances and allowances to be fetched as well as transfers to be verified
//! from a node in a single round-trip, while accounting for pre-interactions.

use {
    super::{BalanceFetching, Query, TransferSimulationError},
    crate::BalanceSimulator,
    alloy_primitives::{Address, U256},
    alloy_provider::{CallItem, MulticallError, Provider, transport::RpcError},
    alloy_sol_types::SolCall,
    anyhow::{Context, Result},
    contracts::ERC20,
    ethrpc::{Web3, alloy::ProviderLabelingExt},
    futures::future,
    itertools::Itertools,
    model::order::SellTokenSource,
};

pub struct Balances {
    web3: Web3,
    balance_simulator: BalanceSimulator,
    /// Hook-free queries read per `aggregate3` call.
    batch_size: usize,
}

impl Balances {
    pub fn new(web3: &Web3, balance_simulator: BalanceSimulator, batch_size: usize) -> Self {
        let web3 = web3.labeled("balanceFetching");

        Self {
            web3,
            balance_simulator,
            batch_size,
        }
    }

    fn vault_relayer(&self) -> Address {
        self.balance_simulator.vault_relayer
    }

    async fn tradable_balance_simulated(&self, query: &Query) -> Result<U256> {
        // Only ERC20 sell-token balances are supported; other sources are
        // deprecated and rejected at order creation.
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

    /// One `aggregate3` for all queries; a failed sub-call fails only its
    /// query.
    async fn aggregate_plain_balances(&self, queries: &[&Query]) -> Result<Vec<Result<U256>>> {
        let mut multicall = self
            .web3
            .provider
            .multicall()
            // `allowance` returns a `uint256` like `balanceOf`, so one decoder fits both.
            .dynamic::<ERC20::ERC20::balanceOfCall>();
        for query in queries {
            let balance = ERC20::ERC20::balanceOfCall {
                account: query.owner,
            };
            let allowance = ERC20::ERC20::allowanceCall {
                owner: query.owner,
                spender: self.vault_relayer(),
            };
            multicall = multicall
                .add_call_dynamic(
                    CallItem::new(query.token, balance.abi_encode().into()).allow_failure(true),
                )
                .add_call_dynamic(
                    CallItem::new(query.token, allowance.abi_encode().into()).allow_failure(true),
                );
        }
        let results = multicall.aggregate3().await?;
        anyhow::ensure!(
            results.len() == 2 * queries.len(),
            "aggregate3 returned {} results",
            results.len()
        );
        Ok(results
            .into_iter()
            .tuples()
            .map(|(balance, allowance)| Ok(std::cmp::min(balance?, allowance?)))
            .collect())
    }

    async fn plain_balance(&self, query: &Query) -> Result<U256> {
        let mut results = self.aggregate_plain_balances(&[query]).await?;
        results.pop().context("empty aggregate3 response")?
    }

    /// Failed queries are retried alone: a token burning all its gas starves
    /// the sub-calls after it (63/64 rule) or runs the whole `aggregate3` out
    /// of gas. A batch that failed for any other reason fails all its queries.
    async fn plain_balances(&self, queries: &[&Query]) -> Vec<Result<U256>> {
        let batch = match self.aggregate_plain_balances(queries).await {
            Ok(results) => Some(results),
            Err(err) if out_of_gas(&err) => {
                tracing::warn!(?err, "batched balance fetch ran out of gas");
                None
            }
            Err(err) => {
                tracing::warn!(?err, "batched balance fetch failed");
                return queries
                    .iter()
                    .map(|_| Err(anyhow::anyhow!("batched balance fetch failed: {err:#}")))
                    .collect();
            }
        };
        let batch = &batch;
        future::join_all(queries.iter().enumerate().map(|(i, query)| async move {
            match batch.as_ref().and_then(|results| results.get(i)) {
                Some(Ok(balance)) => Ok(*balance),
                _ => self.plain_balance(query).await,
            }
        }))
        .await
    }
}

/// Whether the node executed the call and it ran out of gas, as opposed to
/// the call failing before or in transport.
fn out_of_gas(err: &anyhow::Error) -> bool {
    matches!(
        err.downcast_ref::<MulticallError>(),
        Some(MulticallError::TransportError(RpcError::ErrorResp(payload)))
            if payload.message.to_lowercase().contains("gas")
    )
}

#[async_trait::async_trait]
impl BalanceFetching for Balances {
    async fn get_balances(&self, queries: &[Query]) -> Vec<Result<U256>> {
        // Pre-interactions and non-ERC20 sources need the simulator, which
        // rejects the latter.
        let (plain, simulated): (Vec<_>, Vec<_>) =
            queries.iter().enumerate().partition(|(_, query)| {
                query.interactions.is_empty() && query.source == SellTokenSource::Erc20
            });

        let plain = async {
            let plain_queries: Vec<&Query> = plain.iter().map(|(_, query)| *query).collect();
            let batches = plain_queries
                .chunks(self.batch_size)
                .map(|batch| self.plain_balances(batch));
            let results = future::join_all(batches).await.into_iter().flatten();
            plain
                .iter()
                .map(|(i, _)| *i)
                .zip(results)
                .collect::<Vec<_>>()
        };
        let simulated =
            future::join_all(simulated.iter().map(|(i, query)| async move {
                (*i, self.tradable_balance_simulated(query).await)
            }));

        let (mut results, simulated) = future::join(plain, simulated).await;
        results.extend(simulated);
        results.sort_by_key(|(i, _)| *i);
        results.into_iter().map(|(_, result)| result).collect()
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
        // Only ERC20 sell-token balances are supported; other sources are
        // deprecated and rejected at order creation.
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
        alloy_primitives::{Bytes, address},
        alloy_provider::{bindings::IMulticall3, mock::Asserter},
        balance_overrides::DummyStateOverrider,
        contracts::GPv2Settlement,
        ethrpc::Web3,
        model::order::SellTokenSource,
        std::sync::Arc,
    };

    /// A fetcher whose node answers with whatever is pushed onto `asserter`.
    fn mocked_fetcher(asserter: Asserter) -> Balances {
        let web3 = Web3::with_asserter(asserter);
        let settlement =
            GPv2Settlement::GPv2Settlement::new(Address::with_last_byte(1), web3.provider.clone());
        let balances = contracts::support::Balances::Instance::new(
            Address::with_last_byte(2),
            web3.provider.clone(),
        );
        Balances::new(
            &web3,
            BalanceSimulator::new(
                settlement,
                balances,
                Address::with_last_byte(3),
                Arc::new(DummyStateOverrider),
            ),
            100,
        )
    }

    fn query(id: u8) -> Query {
        Query {
            owner: Address::repeat_byte(id),
            token: Address::with_last_byte(id),
            source: SellTokenSource::Erc20,
            interactions: vec![],
            balance_override: None,
        }
    }

    /// An `aggregate3` response with one `uint256` per sub-call; `None` is a
    /// failed sub-call.
    fn aggregate3_response(sub_calls: Vec<Option<u64>>) -> Bytes {
        let results = sub_calls
            .into_iter()
            .map(|value| IMulticall3::Result {
                success: value.is_some(),
                returnData: value
                    .map(U256::from)
                    .map(|value| ERC20::ERC20::balanceOfCall::abi_encode_returns(&value))
                    .unwrap_or_default()
                    .into(),
            })
            .collect();
        IMulticall3::aggregate3Call::abi_encode_returns(&results).into()
    }

    #[tokio::test]
    async fn batches_plain_queries_into_one_aggregate3() {
        let asserter = Asserter::new();
        // (balance, allowance) per query; the third `balanceOf` failed and gets
        // retried alone.
        asserter.push_success(&aggregate3_response(vec![
            Some(10),
            Some(20),
            Some(30),
            Some(5),
            None,
            Some(7),
        ]));
        asserter.push_success(&aggregate3_response(vec![Some(7), Some(8)]));

        let fetcher = mocked_fetcher(asserter);
        let results = fetcher.get_balances(&[query(1), query(2), query(3)]).await;

        assert_eq!(results[0].as_ref().unwrap(), &U256::from(10));
        assert_eq!(results[1].as_ref().unwrap(), &U256::from(5));
        assert_eq!(results[2].as_ref().unwrap(), &U256::from(7));
    }

    #[tokio::test]
    async fn retries_queries_individually_when_the_batch_runs_out_of_gas() {
        let asserter = Asserter::new();
        asserter.push_failure_msg("out of gas");
        asserter.push_success(&aggregate3_response(vec![Some(1), Some(2)]));
        asserter.push_success(&aggregate3_response(vec![Some(3), Some(4)]));

        let fetcher = mocked_fetcher(asserter);
        let results = fetcher.get_balances(&[query(1), query(2)]).await;

        assert_eq!(results[0].as_ref().unwrap(), &U256::from(1));
        assert_eq!(results[1].as_ref().unwrap(), &U256::from(3));
    }

    #[tokio::test]
    async fn does_not_retry_after_other_batch_errors() {
        let asserter = Asserter::new();
        asserter.push_failure_msg("execution reverted");
        // Would be consumed by a retry.
        asserter.push_success(&aggregate3_response(vec![Some(1), Some(2)]));

        let fetcher = mocked_fetcher(asserter);
        let results = fetcher.get_balances(&[query(1), query(2)]).await;

        assert!(results[0].is_err());
        assert!(results[1].is_err());
    }

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
            100,
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
