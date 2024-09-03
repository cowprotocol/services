//! An `eth_call` simulation-based balance reading implementation. This allows
//! balances and allowances to be fetched as well as transfers to be verified
//! from a node in a single round-trip, while accounting for pre-interactions.

use {
    super::{BalanceFetching, Query, TransferSimulationError},
    anyhow::{Context, Result},
    ethcontract::{tokens::Tokenize, Bytes, H160, U256},
    ethrpc::{Web3, Web3CallBatch, MAX_BATCH_SIZE},
    futures::future,
    std::sync::OnceLock,
    web3::{
        ethabi::{self, ParamType, Token},
        types::CallRequest,
    },
};

pub struct Balances {
    balances: contracts::support::Balances,
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

        Self {
            balances: contracts::support::Balances::at(&web3, settlement),
            settlement,
            vault_relayer,
            vault,
        }
    }

    fn call(&self, query: &Query, amount: Option<U256>) -> CallRequest {
        // We simulate the balances from the Settlement contract's context. This
        // allows us to check:
        // 1. How the pre-interactions would behave as part of the settlement
        // 2. Simulate the actual VaultRelayer transfers that would happen as part of a
        //    settlement
        //
        // This allows us to end up with very accurate balance simulations.
        let call_builder = self.balances.methods().balance(
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
        );
        contracts::storage_accessible::call(
            call_builder.tx.to.expect("builder populates to"),
            contracts::bytecode!(contracts::support::Balances),
            call_builder.tx.data.expect("builder populates data"),
        )
    }

    async fn simulate(
        &self,
        queries: impl IntoIterator<Item = (&Query, Option<U256>)>,
    ) -> Vec<Result<Simulation>> {
        // TODO(nlordell): Use `Multicall` here to use fewer node round-trips

        // All these requests are pretty fast and roughly take the same amount
        // of time. By manually batching them together we ensure that the
        // automatic batching logic of the underlying Web3Transport does not
        // sneak slow unrelated requests into these batches which would result
        // in slower execution overall.
        let web3 = self.balances.raw_instance().web3().transport().clone();
        let mut batch = Web3CallBatch::new(web3);
        let futures: Vec<_> = queries
            .into_iter()
            .map(|(query, amount)| batch.push(self.call(query, amount), None))
            .collect();

        batch.execute_all(MAX_BATCH_SIZE).await;

        future::join_all(futures)
            .await
            .into_iter()
            .map(|result| decode_return_data(result?))
            .collect()
    }
}

fn decode_return_data(bytes: web3::types::Bytes) -> Result<Simulation> {
    static KIND: OnceLock<[ParamType; 4]> = OnceLock::new();
    let value = KIND.get_or_init(|| {
        [
            ParamType::Uint(256),
            ParamType::Uint(256),
            ParamType::Uint(256),
            ParamType::Bool,
        ]
    });

    let tokens = ethabi::decode(value, &bytes.0)?;
    let (token_balance, allowance, effective_balance, can_transfer) =
        <(U256, U256, U256, bool)>::from_token(Token::Tuple(tokens))?;

    Ok(Simulation {
        token_balance,
        allowance,
        effective_balance,
        can_transfer,
    })
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
        // TODO(nlordell): Use `Multicall` here to use fewer node round-trips
        self.simulate(queries.iter().map(|query| (query, None)))
            .await
            .into_iter()
            .map(|simulation| {
                let simulation = simulation?;
                Ok(if simulation.can_transfer {
                    simulation.effective_balance
                } else {
                    U256::zero()
                })
            })
            .collect()
    }

    async fn can_transfer(
        &self,
        query: &Query,
        amount: U256,
    ) -> Result<(), TransferSimulationError> {
        let simulation = self
            .simulate([(query, Some(amount))])
            .await
            .pop()
            .context("simulation did not yield a result")?
            .map_err(TransferSimulationError::Other)?;

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
