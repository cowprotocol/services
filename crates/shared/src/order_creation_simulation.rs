use {
    anyhow::anyhow,
    async_trait::async_trait,
    model::order::Order,
    simulator::{
        simulation_builder::{
            self,
            Block,
            ExecutionAmount,
            PriceEncoding,
            SettlementSimulator,
            Solver,
        },
        tenderly,
    },
};

/// Outcome of the order creation simulation.
#[derive(Debug)]
pub enum OrderSimulationError {
    /// The simulation ran and the transaction reverted. `reason` is the
    /// revert string returned by the EVM (or a Tenderly reason string).
    /// `tenderly_request` carries the full payload (calldata, state
    /// overrides, block) needed to replay the simulation manually or against
    /// Tenderly's API, independent of whether `tenderly_url` was produced.
    /// Boxed because the request DTO is large enough that an inline copy
    /// would blow up `Result<(), OrderSimulationError>`'s stack footprint.
    Reverted {
        reason: String,
        tenderly_url: Option<String>,
        tenderly_request: Option<Box<tenderly::dto::Request>>,
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
    #[tracing::instrument(skip_all)]
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
            .presign_orders()
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
        let tenderly_url = match (tenderly, tenderly_request.as_ref()) {
            (Some(api), Some(req)) => api.simulate_and_share(req.clone()).await.ok(),
            _ => None,
        };
        Err(OrderSimulationError::Reverted {
            reason: err.to_string(),
            tenderly_url,
            tenderly_request: tenderly_request.map(Box::new),
        })
    }
}
