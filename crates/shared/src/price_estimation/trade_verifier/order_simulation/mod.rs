use std::sync::Arc;
use contracts::alloy::GPv2Settlement;
use ethrpc::Web3;
use model::{
    DomainSeparator,
    order::Order
};
use anyhow::Result;
use crate::price_estimation::trade_verifier::balance_overrides::BalanceOverriding;

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
    web3: Web3,
    settlement: GPv2Settlement::Instance,
    balance_overrider: Arc<dyn BalanceOverriding>,
}

impl OrderExecutionSimulator {

}