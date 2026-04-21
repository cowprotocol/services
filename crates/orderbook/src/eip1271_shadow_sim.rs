use {
    crate::order_simulator::{self, OrderSimulator},
    async_trait::async_trait,
    model::order::Order,
    shared::order_validation::{Eip1271ShadowSimulator, ShadowSimError},
    std::sync::Arc,
};

/// Adapter exposing `OrderSimulator` via the
/// `shared::order_validation::Eip1271ShadowSimulator` trait.
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
impl Eip1271ShadowSimulator for OrderSimulatorAdapter {
    async fn simulate(&self, order: &Order) -> Result<(), ShadowSimError> {
        let swap = self
            .inner
            .encode_order(order, Vec::new(), None)
            .await
            .map_err(map_simulator_err)?;
        let result = self
            .inner
            .simulate_swap(swap, None)
            .await
            .map_err(map_simulator_err)?;
        match result.error {
            None => Ok(()),
            Some(reason) => Err(ShadowSimError::Reverted {
                reason,
                tenderly_url: result.tenderly_url,
            }),
        }
    }
}

fn map_simulator_err(err: order_simulator::Error) -> ShadowSimError {
    match err {
        order_simulator::Error::Other(e) | order_simulator::Error::MalformedInput(e) => {
            ShadowSimError::Infra(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn impls_trait() {
        fn assert_impl<T: Eip1271ShadowSimulator>() {}
        assert_impl::<OrderSimulatorAdapter>();
    }
}
