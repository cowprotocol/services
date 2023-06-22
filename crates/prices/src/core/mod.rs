//! Core logic of the prices service. TODO Write more what constitutes "core
//! logic".

// TODO Remove this ASAP
#![allow(dead_code)]

mod estimate;
pub mod estimator;
pub mod eth;
pub mod swap;

pub use {
    estimate::{estimate, Deadline, Estimate},
    estimator::Estimator,
    swap::Swap,
};
