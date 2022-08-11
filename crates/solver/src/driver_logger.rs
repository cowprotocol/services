use crate::{
    metrics::SolverMetrics,
    settlement::Settlement,
    settlement_simulation::{simulate_before_after_access_list, TenderlyApi},
};
use anyhow::{Context, Result};
use itertools::Itertools;
use model::order::{Order, OrderKind};
use primitive_types::H256;
use shared::Web3;
use std::sync::Arc;

pub struct DriverLogger {
    pub metrics: Arc<dyn SolverMetrics>,
    pub web3: Web3,
    pub tenderly: Option<TenderlyApi>,
    pub network_id: String,
}

impl DriverLogger {
    pub async fn metric_access_list_gas_saved(&self, transaction_hash: H256) -> Result<()> {
        let gas_saved = simulate_before_after_access_list(
            &self.web3,
            self.tenderly.as_ref().context("tenderly disabled")?,
            self.network_id.clone(),
            transaction_hash,
        )
        .await?;
        tracing::debug!(?gas_saved, "access list gas saved");
        if gas_saved.is_sign_positive() {
            self.metrics
                .settlement_access_list_saved_gas(gas_saved, "positive");
        } else {
            self.metrics
                .settlement_access_list_saved_gas(-gas_saved, "negative");
        }

        Ok(())
    }

    /// Collects all orders which got traded in the settlement. Tapping into partially fillable
    /// orders multiple times will not result in duplicates. Partially fillable orders get
    /// considered as traded only the first time we tap into their liquidity.
    pub fn get_traded_orders(settlement: &Settlement) -> Vec<Order> {
        let mut traded_orders = Vec::new();
        for (_, group) in &settlement
            .executed_trades()
            .map(|(trade, _)| trade)
            .group_by(|trade| trade.order.metadata.uid)
        {
            let mut group = group.into_iter().peekable();
            let order = &group.peek().unwrap().order;
            let was_already_filled = match order.data.kind {
                OrderKind::Buy => &order.metadata.executed_buy_amount,
                OrderKind::Sell => &order.metadata.executed_sell_amount,
            } > &0u8.into();
            let is_getting_filled = group.any(|trade| !trade.executed_amount.is_zero());
            if !was_already_filled && is_getting_filled {
                traded_orders.push(order.clone());
            }
        }
        traded_orders
    }
}
