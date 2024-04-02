use super::{auction::GasPrice, eth::Gas};

/// Parameters that define the possibility of a revert when executing a
/// solution.
#[derive(Debug, Default, Clone)]
pub struct Risk {
    pub gas_amount_factor: f64,
    pub gas_price_factor: f64,
    pub nmb_orders_factor: f64,
    pub intercept: f64,
}

impl Risk {
    pub fn success_probability(
        &self,
        gas_amount: Gas,
        gas_price: GasPrice,
        nmb_orders: usize,
    ) -> f64 {
        let exponent = -self.intercept
            - self.gas_amount_factor * gas_amount.0.to_f64_lossy() / 1_000_000.
            - self.gas_price_factor * gas_price.0 .0.to_f64_lossy() / 10_000_000_000.
            - self.nmb_orders_factor * nmb_orders as f64;
        1. / (1. + exponent.exp())
    }
}
