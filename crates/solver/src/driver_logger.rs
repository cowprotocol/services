use crate::{
    driver::solver_settlements::RatedSettlement,
    metrics::SolverMetrics,
    settlement::Settlement,
    settlement_simulation::{simulate_before_after_access_list, TenderlyApi},
    settlement_submission::SubmissionError,
    solver::Solver,
};
use anyhow::{Context, Result};
use itertools::Itertools;
use model::order::{Order, OrderKind};
use primitive_types::H256;
use shared::Web3;
use std::sync::Arc;
use web3::types::TransactionReceipt;

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
    fn get_traded_orders(settlement: &Settlement) -> Vec<Order> {
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

    pub async fn log_submission_info(
        &self,
        submission: &Result<TransactionReceipt, SubmissionError>,
        rated_settlement: &RatedSettlement,
        solver: &Arc<dyn Solver>,
    ) {
        self.metrics
            .settlement_revertable_status(rated_settlement.settlement.revertable(), solver.name());
        match submission {
            Ok(receipt) => {
                let name = solver.name();
                tracing::info!(
                    settlement_id =% rated_settlement.id,
                    transaction_hash =? receipt.transaction_hash,
                    "Successfully submitted settlement",
                );
                Self::get_traded_orders(&rated_settlement.settlement)
                    .iter()
                    .for_each(|order| self.metrics.order_settled(order, name));
                self.metrics.settlement_submitted(
                    crate::metrics::SettlementSubmissionOutcome::Success,
                    name,
                );
                if let Err(err) = self
                    .metric_access_list_gas_saved(receipt.transaction_hash)
                    .await
                {
                    tracing::debug!(?err, "access list metric not saved");
                }
                match receipt.effective_gas_price {
                    Some(price) => {
                        self.metrics.transaction_gas_price(price);
                    }
                    None => {
                        tracing::error!("node did not return effective gas price in tx receipt");
                    }
                }
            }
            Err(err) => {
                // Since we simulate and only submit solutions when they used to pass before, there is no
                // point in logging transaction failures in the form of race conditions as hard errors.
                tracing::warn!(
                    settlement_id =% rated_settlement.id, ?err,
                    "Failed to submit settlement",
                );
                self.metrics
                    .settlement_submitted(err.as_outcome(), solver.name());
                if let Some(transaction_hash) = err.transaction_hash() {
                    if let Err(err) = self.metric_access_list_gas_saved(transaction_hash).await {
                        tracing::debug!(?err, "access list metric not saved");
                    }
                }
            }
        }
    }
}
