use crate::encoded_settlement::{EncodedSettlement, encode_trade};
use crate::price_estimation::trade_verifier::balance_overrides::{
    BalanceOverrideRequest, BalanceOverriding,
};
use crate::tenderly_api::TenderlyCodeSimulator;

use alloy::primitives::{Address, Bytes, TxKind, U256};
use alloy::providers::Provider;
use alloy::rpc::types::{
    BlockId, BlockNumberOrTag, TransactionInput, TransactionRequest, state::StateOverride,
};
use alloy::sol_types::SolCall;
use anyhow::{Context, Result};
use contracts::alloy::GPv2Settlement;

use crate::trade_finding::Interaction;
use model::order::OrderData;
use model::{DomainSeparator, order::Order};
use std::sync::Arc;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FlashLoanParams {
    pub address: Address,
    pub amount: U256,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WrapperParams {
    pub address: Address,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SimulationOptions {
    pub pre_interactions: Vec<Interaction>,
    pub post_interactions: Vec<Interaction>,
    pub flash_loan: Option<FlashLoanParams>,
    pub wrapper: Option<WrapperParams>,
}

/// A component that can simulate the execution of an order.
#[async_trait::async_trait]
pub trait OrderExecutionSimulating: Send + Sync {
    /// Simulates the execution of an order.
    async fn simulate_order_execution(
        &self,
        order: &Order,
        domain_separator: &DomainSeparator,
        options: SimulationOptions,
    ) -> Result<()>;
}

pub struct OrderExecutionSimulator {
    settlement: GPv2Settlement::Instance,
    balance_overrider: Arc<dyn BalanceOverriding>,
    simulator: Option<Arc<TenderlyCodeSimulator>>,
}

impl OrderExecutionSimulator {
    pub fn new(
        settlement: GPv2Settlement::Instance,
        balance_overrider: Arc<dyn BalanceOverriding>,
        simulator: Option<Arc<TenderlyCodeSimulator>>,
    ) -> Self {
        Self {
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
        options: SimulationOptions,
    ) -> Result<EncodedSettlement> {
        let tokens = {
            let mut tokens = vec![order.data.sell_token, order.data.buy_token];
            tokens.sort();
            tokens.dedup();
            tokens
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
        // For same-token trades, the price is 1:1.
        let clearing_prices = if tokens.len() == 1 {
            vec![U256::from(1)]
        } else {
            let mut prices = vec![U256::ZERO; tokens.len()];
            prices[sell_token_index] = order.data.buy_amount;
            prices[buy_token_index] = order.data.sell_amount;
            prices
        };

        let trade = encode_trade(
            &order.data,
            &order.signature,
            order.metadata.owner,
            sell_token_index,
            buy_token_index,
            order.data.sell_amount,
        );

        let encoded_pre_interactions = options
            .pre_interactions
            .into_iter()
            .map(|i| i.encode())
            .collect();

        let encoded_post_interactions = options
            .post_interactions
            .into_iter()
            .map(|i| i.encode())
            .collect();

        Ok(EncodedSettlement {
            tokens,
            clearing_prices,
            trades: vec![trade],
            interactions: [
                encoded_pre_interactions,
                Vec::new(),
                encoded_post_interactions,
            ],
        })
    }
}

#[async_trait::async_trait]
impl OrderExecutionSimulating for OrderExecutionSimulator {
    async fn simulate_order_execution(
        &self,
        order: &Order,
        domain_separator: &DomainSeparator,
        options: SimulationOptions,
    ) -> Result<()> {
        let settlement = self.encode_settlement(order, domain_separator, options.clone())?;
        let overrides = self.prepare_state_overrides(&order.data).await;

        let settle_call = GPv2Settlement::GPv2Settlement::settleCall {
            tokens: settlement.tokens,
            clearingPrices: settlement.clearing_prices,
            trades: settlement.trades.into_iter().map(Into::into).collect(),
            interactions: settlement
                .interactions
                .map(|i| i.into_iter().map(Into::into).collect()),
        };

        let settle_calldata = settle_call.abi_encode();

        // Handle Wrapping if present
        let (to, calldata) = if let Some(wrapper) = options.wrapper {
            let wrapped_call = contracts::alloy::ICowWrapper::ICowWrapper::wrappedSettleCall {
                settleData: settle_calldata.into(),
                wrapperData: wrapper.data.into(),
            };
            (wrapper.address, wrapped_call.abi_encode())
        } else if let Some(_) = options.flash_loan {
            tracing::warn!(
                "Flashloan simulation requested but using direct settlement independent of flashloan."
            );
            (*self.settlement.address(), settle_calldata)
        } else {
            (*self.settlement.address(), settle_calldata)
        };

        let tx = TransactionRequest {
            from: Some(order.metadata.owner),
            to: Some(TxKind::Call(to)),
            input: TransactionInput::new(calldata.into()),
            value: Some(U256::ZERO),
            ..Default::default()
        };

        if let Some(tenderly) = &self.simulator {
            if let Err(err) = tenderly.log_simulation_command(tx.clone(), overrides.clone(), None) {
                tracing::debug!(?err, "could not log tenderly simulation command");
            }
        }

        let _: Bytes = self
            .settlement
            .provider()
            .raw_request(
                "eth_call".into(),
                (tx, BlockId::Number(BlockNumberOrTag::Latest), overrides),
            )
            .await
            .context(format!(
                "failed to execute settlement for order: {:?}",
                order
            ))?;

        Ok(())
    }
}
