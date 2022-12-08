// TODO Remove dead_code ASAP
#![allow(dead_code)]
#![forbid(unsafe_code)]

pub mod arguments;
pub mod auction_converter;
pub mod commit_reveal;
pub mod driver;
pub mod settlement_proposal;

// TODO api doesn't need to be pub
pub mod api;
mod logic;
mod solver;
pub mod util;
