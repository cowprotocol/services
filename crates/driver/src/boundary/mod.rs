//! This is a very simple anti-corruption layer between the driver and the rest
//! of the codebase. The purpose of this layer is to give a very clear
//! indication of where and how the integration between the driver and the rest
//! of the code happens, and to serve as a line of defense against leaking
//! unnecessary details from that codebase into the driver.
//!
//! https://gist.github.com/ennmichael/0f68a4e0c33df80f9d415b20f9848bcf

pub use {
    contracts,
    solver::{interactions::allowances::Approval, settlement::Settlement},
};
