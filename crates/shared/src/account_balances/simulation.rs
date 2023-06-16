//! An `eth_call` simulation-based balance reading implementation. This allows
//! balances and allowances to be fetched as well as transfers to be verified
//! from a node in a single round-trip, while accounting for pre-interactions.

use {
    super::{BalanceFetching, Query, TransferSimulationError},
    crate::{code_simulation::CodeSimulating, ethrpc::extensions::StateOverride},
    anyhow::{Context, Result},
    ethcontract::{tokens::Tokenize, Bytes, H160, U256},
    futures::future,
    maplit::hashmap,
    model::order::SellTokenSource,
    std::sync::Arc,
    web3::{ethabi::Token, types::CallRequest},
};

pub struct Balances {
    simulator: Arc<dyn CodeSimulating>,
    settlement: H160,
    vault_relayer: H160,
    vault: H160,
}

impl Balances {
    pub fn new(
        simulator: Arc<dyn CodeSimulating>,
        settlement: H160,
        vault_relayer: H160,
        vault: Option<H160>,
    ) -> Self {
        // Note that the balances simulation **will fail** if the `vault`
        // address is not a contract and the `source` is set to one of
        // `SellTokenSource::{External, Internal}` (i.e. the Vault contract is
        // needed). This is because Solidity generates code to verify that
        // contracts exist at addresses that get called. This allows us to
        // properly check if the `source` is not supported for the deployment
        // work without additional code paths :tada:!
        let vault = vault.unwrap_or_default();

        Self {
            simulator,
            settlement,
            vault_relayer,
            vault,
        }
    }

    async fn simulate(&self, query: &Query, amount: Option<U256>) -> Result<Simulation> {
        // We simulate the balances from the Settlement contract's context. This
        // allows us to check:
        // 1. How the pre-interactions would behave as part of the settlement
        // 2. Simulate the actual VaultRelayer transfers that would happen as
        //    part of a settlement
        //
        // This allows us to end up with very accurate balance simulations.
        let balances = dummy_contract!(contracts::support::Balances, self.settlement);
        let tx = balances
            .methods()
            .balance(
                (self.settlement, self.vault_relayer, self.vault),
                query.owner,
                query.token,
                amount.unwrap_or_default(),
                Bytes(query.source.as_bytes()),
                vec![],
            )
            .tx;

        let call = CallRequest {
            to: tx.to,
            data: tx.data,
            ..Default::default()
        };
        let overrides = hashmap! {
            balances.address() => StateOverride {
                code: Some(deployed_bytecode!(contracts::support::Balances)),
                ..Default::default()
            },
        };

        let output = self.simulator.simulate(call, overrides).await?;
        let simulation = Simulation::decode(&output)?;

        tracing::trace!(?query, ?amount, ?simulation, "simulated balances");
        Ok(simulation)
    }
}

#[derive(Debug)]
struct Simulation {
    token_balance: U256,
    allowance: U256,
    effective_balance: U256,
    can_transfer: bool,
}

impl Simulation {
    fn decode(output: &[u8]) -> Result<Self> {
        let function = contracts::support::Balances::raw_contract()
            .abi
            .function("balance")
            .unwrap();
        let tokens = function.decode_output(output).context("decode")?;
        let (token_balance, allowance, effective_balance, can_transfer) =
            Tokenize::from_token(Token::Tuple(tokens))?;

        Ok(Self {
            token_balance,
            allowance,
            effective_balance,
            can_transfer,
        })
    }
}

#[async_trait::async_trait]
impl BalanceFetching for Balances {
    async fn get_balances(&self, queries: &[Query]) -> Vec<Result<U256>> {
        let futures = queries
            .iter()
            .map(|query| async {
                let simulation = self.simulate(query, None).await?;
                Ok(if simulation.can_transfer {
                    simulation.effective_balance
                } else {
                    U256::zero()
                })
            })
            .collect::<Vec<_>>();

        future::join_all(futures).await
    }

    async fn can_transfer(
        &self,
        token: H160,
        owner: H160,
        amount: U256,
        source: SellTokenSource,
    ) -> Result<(), TransferSimulationError> {
        let simulation = self
            .simulate(
                &Query {
                    owner,
                    token,
                    source,
                },
                Some(amount),
            )
            .await?;

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
    };

    #[ignore]
    #[tokio::test]
    async fn test_for_user() {
        let balances = Balances::new(
            Arc::new(Web3::new(ethrpc::create_env_test_transport())),
            addr!("9008d19f58aabd9ed0d60971565aa8510560ab41"),
            addr!("C92E8bdf79f0507f65a392b0ab4667716BFE0110"),
            Some(addr!("BA12222222228d8Ba445958a75a0704d566BF2C8")),
        );

        let owner = addr!("b0a4e99371dfb0734f002ae274933b4888f618ef");
        let token = addr!("d909c5862cdb164adb949d92622082f0092efc3d");
        let amount = 50000000000000000000000_u128.into();
        let source = SellTokenSource::Erc20;

        balances
            .can_transfer(token, owner, amount, source)
            .await
            .unwrap();
        println!("{owner} can transfer {amount} {token}!");
    }
}
