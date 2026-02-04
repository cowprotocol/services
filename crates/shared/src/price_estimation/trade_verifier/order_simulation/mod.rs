use std::sync::Arc;
use alloy::primitives::U256;
use alloy::rpc::types::state::StateOverride;
use contracts::alloy::GPv2Settlement;
use ethrpc::Web3;
use model::{
    DomainSeparator,
    order::Order
};
use anyhow::{Context, Result};
use model::order::OrderData;
use crate::encoded_settlement::{encode_trade, EncodedSettlement};
use crate::tenderly_api::TenderlyCodeSimulator;
use crate::price_estimation::trade_verifier::balance_overrides::{BalanceOverrideRequest, BalanceOverriding};

/// A component that can simulate the execution of an order.
#[async_trait::async_trait]
pub trait OrderExecutionSimulating: Send + Sync {
    /// Simulates the execution of an order.
    async fn simulate_order_execution(
        &self,
        order: &Order,
        domain_separator: &DomainSeparator,
    ) -> Result<()>;
}

pub struct OrderExecutionSimulator {
    #[expect(dead_code)]
    web3: Web3,
    settlement: GPv2Settlement::Instance,
    balance_overrider: Arc<dyn BalanceOverriding>,
    simulator: Option<Arc<TenderlyCodeSimulator>>,
}

impl OrderExecutionSimulator {
    pub fn new(
        web3: Web3,
        settlement: GPv2Settlement::Instance,
        balance_overrider: Arc<dyn BalanceOverriding>,
        simulator: Option<Arc<TenderlyCodeSimulator>>,
    ) -> Self {
        Self {
            web3,
            settlement,
            balance_overrider,
            simulator,
        }
    }

    /// Prepares the state overrides for the simulation.
    ///
    /// This will override the `buy_token` balance of the settlement contract
    /// to ensure that it can pay out the `buy_token` to the receiver.
    async fn prepare_state_overrides(&self, order: &OrderData) -> StateOverride {
        let request = BalanceOverrideRequest {
            token: order.buy_token,
            holder: *self.settlement.address(),
            amount: order.buy_amount,
        };

        self.balance_overrider
            .state_override(request)
            .await
            .into_iter()
            .collect()
    }

    /// Encodes a settlement call that settles the given order at its limit price.
    fn encode_settlement(
        &self,
        order: &Order,
        _domain_separator: &DomainSeparator,
    ) -> Result<EncodedSettlement> {
        let tokens = if order.data.sell_token < order.data.buy_token {
            vec![order.data.sell_token, order.data.buy_token]
        } else {
            vec![order.data.buy_token, order.data.sell_token]
        };

        let sell_token_index = tokens
            .iter()
            .position(|&t| t == order.data.sell_token)
            .unwrap();

        let buy_token_index = tokens
            .iter()
            .position(|&t| t == order.data.buy_token)
            .unwrap();

        // Clearing prices are set such that the order is settled exactly at its limit price.
        let mut clearing_prices = vec![U256::ZERO; 2];
        clearing_prices[sell_token_index] = order.data.buy_amount;
        clearing_prices[buy_token_index] = order.data.sell_amount;

        let trade = encode_trade(
            &order.data,
            &order.signature,
            order.metadata.owner,
            sell_token_index,
            buy_token_index,
            order.data.sell_amount,
        );

        Ok(EncodedSettlement {
            tokens,
            clearing_prices,
            trades: vec![trade],
            interactions: Default::default(),
        })
    }
}

#[async_trait::async_trait]
impl OrderExecutionSimulating for OrderExecutionSimulator {
    async fn simulate_order_execution(
        &self,
        order: &Order,
        domain_separator: &DomainSeparator,
    ) -> Result<()> {
        let settlement = self.encode_settlement(order, domain_separator)?;
        let overrides = self.prepare_state_overrides(&order.data).await;

        let call = GPv2Settlement::GPv2Settlement::settleCall {
            tokens: settlement.tokens,
            clearingPrices: settlement.clearing_prices,
            trades: settlement.trades.into_iter().map(Into::into).collect(),
            interactions: settlement
                .interactions
                .map(|i| i.into_iter().map(Into::into).collect()),
        };

        let settle_simulation = self
            .settlement
            .settle(
                call.tokens,
                call.clearingPrices,
                call.trades,
                call.interactions,
            )
            .state(overrides.clone());

        if let Some(tenderly) = &self.simulator
            && let Err(err) = tenderly.log_simulation_command(
                settle_simulation.clone().into_transaction_request(),
                overrides,
                None, // Use latest block
            )
        {
            tracing::debug!(?err, "could not log tenderly simulation command");
        }

        settle_simulation
            .call()
            .await
            .context(format!("failed to execute settlement for order: {:?}", order))?;

        Ok(())
    }
}