use crate::{
    metrics::SolverMetrics,
    settlement_simulation::{simulate_before_after_access_list, TenderlyApi},
};
use anyhow::{Context, Result};
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
}
