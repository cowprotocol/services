//! Core logic of the prices service. TODO Write more what constitutes "core
//! logic".

// TODO Implement automatic wrapping for ETH

mod estimate;
pub mod eth;
mod price;
pub mod swap;

pub use {
    estimate::{estimate, Estimate, Estimator, EstimatorError},
    price::Price,
    swap::Swap,
};
