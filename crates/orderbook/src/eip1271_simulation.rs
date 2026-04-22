use {
    crate::order_simulator::{self, OrderSimulator},
    async_trait::async_trait,
    model::order::Order,
    shared::order_validation::{Eip1271Simulating, Eip1271SimulationError},
    std::sync::Arc,
};

/// Adapter exposing `OrderSimulator` via the
/// `shared::order_validation::Eip1271Simulating` trait.
///
/// This is a temporary shim. Once the `simulator` crate is refactored to own
/// `OrderSimulator`, `OrderValidator` can depend on it directly and this
/// adapter can be deleted.
pub struct OrderSimulatorAdapter {
    inner: Arc<OrderSimulator>,
}

impl OrderSimulatorAdapter {
    pub fn new(inner: Arc<OrderSimulator>) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl Eip1271Simulating for OrderSimulatorAdapter {
    async fn simulate(&self, order: &Order) -> Result<(), Eip1271SimulationError> {
        let swap = self.inner.encode_order(order, Vec::new(), None).await?;
        let result = self.inner.simulate_swap(swap, None).await?;
        match result.error {
            Some(reason) => Err(Eip1271SimulationError::Reverted {
                reason,
                tenderly_url: result.tenderly_url,
            }),
            None => Ok(()),
        }
    }
}

impl From<order_simulator::Error> for Eip1271SimulationError {
    fn from(err: order_simulator::Error) -> Self {
        match err {
            order_simulator::Error::Other(e) | order_simulator::Error::MalformedInput(e) => {
                Self::Infra(e)
            }
        }
    }
}
