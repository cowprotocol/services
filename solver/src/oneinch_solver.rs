//! Module containing implementation of the 1Inch solver.
//!
//! This simple solver will simply use the 1Inch API to get a quote for a
//! single GPv2 order and produce a settlement directly against 1Inch.

pub mod api;

use self::api::{Amount, OneInchClient, Slippage, Swap, SwapQuery};
use crate::{
    encoding::EncodedInteraction,
    interactions::Erc20ApproveInteraction,
    liquidity::{slippage::MAX_SLIPPAGE_BPS, LimitOrder},
    settlement::{Interaction, Settlement},
    single_order_solver::SingleOrderSolving,
};
use anyhow::{ensure, Result};
use contracts::{GPv2Settlement, ERC20};
use ethcontract::{dyns::DynWeb3, Bytes, U256};
use maplit::hashmap;
use model::order::OrderKind;
use std::{
    collections::HashSet,
    fmt::{self, Display, Formatter},
    iter,
};

/// A GPv2 solver that matches GP **sell** orders to direct 1Inch swaps.
#[derive(Debug)]
pub struct OneInchSolver {
    settlement_contract: GPv2Settlement,
    client: OneInchClient,
    disabled_protocols: HashSet<String>,
}

/// Chain ID for Mainnet.
const MAINNET_CHAIN_ID: u64 = 1;

impl OneInchSolver {
    /// Creates a new 1Inch solver instance for specified settlement contract
    /// instance.
    pub fn new(settlement_contract: GPv2Settlement, chain_id: u64) -> Result<Self> {
        Self::with_disabled_protocols(settlement_contract, chain_id, iter::empty())
    }

    /// Creates a new 1Inch solver with a list of disabled protocols.
    pub fn with_disabled_protocols(
        settlement_contract: GPv2Settlement,
        chain_id: u64,
        disabled_protocols: impl IntoIterator<Item = String>,
    ) -> Result<Self> {
        ensure!(
            chain_id == MAINNET_CHAIN_ID,
            "1Inch solver only supported on Mainnet",
        );

        Ok(Self {
            settlement_contract,
            client: Default::default(),
            disabled_protocols: disabled_protocols.into_iter().collect(),
        })
    }

    /// Gets the list of supported protocols for the 1Inch solver.
    async fn supported_protocols(&self) -> Result<Option<Vec<String>>> {
        let protocols = if self.disabled_protocols.is_empty() {
            None
        } else {
            Some(
                self.client
                    .get_protocols()
                    .await?
                    .protocols
                    .into_iter()
                    .filter(|protocol| !self.disabled_protocols.contains(protocol))
                    .collect(),
            )
        };
        Ok(protocols)
    }

    /// Settles a single sell order against a 1Inch swap using the spcified
    /// protocols.
    async fn settle_order(
        &self,
        order: LimitOrder,
        protocols: Option<Vec<String>>,
    ) -> Result<Settlement> {
        debug_assert_eq!(
            order.kind,
            OrderKind::Sell,
            "only sell orders should be passed to settle_order"
        );

        let spender = self.client.get_spender().await?;
        let sell_token = ERC20::at(&self.web3(), order.sell_token);
        let existing_allowance = sell_token
            .allowance(self.settlement_contract.address(), spender.address)
            .call()
            .await?;

        let query = SwapQuery {
            from_token_address: order.sell_token,
            to_token_address: order.buy_token,
            amount: order.sell_amount,
            from_address: self.settlement_contract.address(),
            slippage: Slippage::basis_points(MAX_SLIPPAGE_BPS).unwrap(),
            protocols,
            // Disable balance/allowance checks, as the settlement contract
            // does not hold balances to traded tokens.
            disable_estimate: Some(true),
            // Use at most 2 connector tokens
            complexity_level: Some(Amount::new(2).unwrap()),
            // Cap swap gas to 750K.
            gas_limit: Some(750_000),
            // Use only 3 main route for cheaper trades.
            main_route_parts: Some(Amount::new(3).unwrap()),
            parts: Some(Amount::new(3).unwrap()),
        };

        tracing::debug!("querying 1Inch swap api with {:?}", query);
        let swap = self.client.get_swap(query).await?;

        ensure!(
            swap.to_token_amount >= order.buy_amount,
            "order limit price not respected",
        );
        let mut settlement = Settlement::new(hashmap! {
            order.sell_token => swap.to_token_amount,
            order.buy_token => swap.from_token_amount,
        });

        settlement.with_liquidity(&order, order.sell_amount)?;

        if existing_allowance < order.sell_amount {
            settlement
                .encoder
                .append_to_execution_plan(Erc20ApproveInteraction {
                    token: sell_token,
                    spender: spender.address,
                    amount: U256::MAX,
                });
        }
        settlement.encoder.append_to_execution_plan(swap);

        Ok(settlement)
    }

    fn web3(&self) -> DynWeb3 {
        self.settlement_contract.raw_instance().web3()
    }
}

impl Interaction for Swap {
    fn encode(&self) -> Vec<EncodedInteraction> {
        vec![(self.tx.to, self.tx.value, Bytes(self.tx.data.clone()))]
    }
}

#[async_trait::async_trait]
impl SingleOrderSolving for OneInchSolver {
    async fn settle_order(&self, order: LimitOrder) -> Result<Settlement> {
        ensure!(
            order.kind == OrderKind::Sell,
            "1Inch only supports sell orders"
        );
        let protocols = self.supported_protocols().await?;
        self.settle_order(order, protocols).await
    }

    fn name(&self) -> &'static str {
        "1Inch"
    }
}

impl Display for OneInchSolver {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "OneInchSolver")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{liquidity::LimitOrder, testutil};
    use contracts::{GPv2Settlement, WETH9};
    use ethcontract::H160;
    use model::order::{Order, OrderCreation, OrderKind};

    fn dummy_solver() -> OneInchSolver {
        let web3 = testutil::dummy_web3();
        let settlement = GPv2Settlement::at(&web3, H160::zero());
        OneInchSolver::new(settlement, MAINNET_CHAIN_ID).unwrap()
    }

    #[tokio::test]
    #[cfg(debug_assertions)]
    #[should_panic]
    async fn panics_when_settling_buy_orders() {
        let _ = dummy_solver()
            .settle_order(
                LimitOrder {
                    kind: OrderKind::Buy,
                    ..Default::default()
                },
                None,
            )
            .await;
    }

    #[tokio::test]
    async fn returns_none_when_no_protocols_are_disabled() {
        let protocols = dummy_solver().supported_protocols().await.unwrap();
        assert!(protocols.is_none());
    }

    #[tokio::test]
    #[ignore]
    async fn filters_disabled_protocols() {
        let mut solver = dummy_solver();

        let all_protocols = solver.client.get_protocols().await.unwrap().protocols;

        solver.disabled_protocols.insert(all_protocols[0].clone());
        let filtered_protocols = solver.supported_protocols().await.unwrap().unwrap();

        assert_eq!(all_protocols[1..], filtered_protocols[..]);
    }

    #[tokio::test]
    #[ignore]
    async fn solve_order_on_oneinch() {
        let web3 = testutil::infura("mainnet");
        let chain_id = web3.eth().chain_id().await.unwrap().as_u64();
        let settlement = GPv2Settlement::deployed(&web3).await.unwrap();

        let weth = WETH9::deployed(&web3).await.unwrap();
        let gno = shared::addr!("6810e776880c02933d47db1b9fc05908e5386b96");

        let solver =
            OneInchSolver::with_disabled_protocols(settlement, chain_id, vec!["PMM1".to_string()])
                .unwrap();
        let settlement = solver
            .settle_order(
                Order {
                    order_creation: OrderCreation {
                        sell_token: weth.address(),
                        buy_token: gno,
                        sell_amount: 1_000_000_000_000_000_000u128.into(),
                        buy_amount: 1u128.into(),
                        kind: OrderKind::Sell,
                        ..Default::default()
                    },
                    ..Default::default()
                }
                .into(),
                None,
            )
            .await
            .unwrap();

        println!("{:#?}", settlement);
    }

    #[tokio::test]
    #[ignore]
    async fn returns_error_on_non_mainnet() {
        let web3 = testutil::infura("rinkeby");
        let chain_id = web3.eth().chain_id().await.unwrap().as_u64();
        let settlement = GPv2Settlement::deployed(&web3).await.unwrap();

        assert!(OneInchSolver::new(settlement, chain_id).is_err())
    }
}
