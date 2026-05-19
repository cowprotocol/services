//! Order-creation simulation recipe.
//!
//! Lives next to its consumer (`order_validation`) rather than inside the
//! `simulator` crate. The `simulator` crate is a flexible builder; this
//! module hard-codes the specific options the orderbook uses at order
//! creation (full fill, fake solver, sufficient buy-token override,
//! Tenderly-on-revert).

use {
    anyhow::anyhow,
    async_trait::async_trait,
    model::order::Order,
    simulator::simulation_builder::{
        self,
        Block,
        ExecutionAmount,
        PriceEncoding,
        SettlementSimulator,
        Solver,
    },
};

/// Outcome of the order creation simulation.
#[derive(Debug)]
pub enum OrderSimulationError {
    /// The simulation ran and the transaction reverted. `reason` is the
    /// revert string returned by the EVM (or a Tenderly reason string).
    Reverted {
        reason: String,
        tenderly_url: Option<String>,
    },
    /// The simulation could not run (RPC failure, Tenderly error, malformed
    /// input, timeout). Treated as fail-open.
    Infra(anyhow::Error),
}

/// Simulates an order's pre-hooks, swap, and post-hooks against the chain.
#[cfg_attr(any(test, feature = "test-util"), mockall::automock)]
#[async_trait]
pub trait OrderSimulating: Send + Sync {
    async fn simulate(
        &self,
        order: &Order,
        full_app_data: &str,
    ) -> Result<(), OrderSimulationError>;
}

/// Drives [`SettlementSimulator`] to run a full-order simulation at order
/// creation time, including pre/post hooks, swap, and any wrapper chain.
pub struct OrderCreationSimulator {
    inner: SettlementSimulator,
}

impl OrderCreationSimulator {
    pub fn new(inner: SettlementSimulator) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl OrderSimulating for OrderCreationSimulator {
    async fn simulate(
        &self,
        order: &Order,
        full_app_data: &str,
    ) -> Result<(), OrderSimulationError> {
        let sim_order = simulation_builder::Order::new(order.data)
            .with_signature(order.metadata.owner, order.signature.clone())
            .fill_at(ExecutionAmount::Full, PriceEncoding::LimitPrice);

        let inputs = self
            .inner
            .new_simulation_builder()
            .with_orders([sim_order])
            .parameters_from_app_data(full_app_data)
            .map_err(|err| OrderSimulationError::Infra(anyhow!(err).context("parse app data")))?
            .from_solver(Solver::Fake(None))
            .provide_sufficient_buy_tokens()
            .at_block(Block::Latest)
            .build()
            .await
            .map_err(|err| OrderSimulationError::Infra(anyhow!(err).context("build")))?;

        // Capture the Tenderly handle and the diagnostic request before
        // consuming `inputs` with `simulate()`. The Tenderly call is only
        // dispatched on revert, since the URL is only useful for diagnostics
        // and most simulations succeed.
        let tenderly = inputs.simulator.tenderly();
        let tenderly_request = inputs.to_tenderly_request().ok();

        let Err(err) = inputs.simulate().await else {
            return Ok(());
        };
        let tenderly_url = match (tenderly, tenderly_request) {
            (Some(api), Some(req)) => api.simulate_and_share(req).await.ok(),
            _ => None,
        };
        Err(OrderSimulationError::Reverted {
            reason: err.to_string(),
            tenderly_url,
        })
    }
}
