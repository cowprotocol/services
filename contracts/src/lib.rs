#[cfg(feature = "bin")]
pub mod paths;

include!(concat!(env!("OUT_DIR"), "/IERC20.rs"));
include!(concat!(env!("OUT_DIR"), "/IUniswapV2Router02.rs"));
include!(concat!(env!("OUT_DIR"), "/GPv2Settlement.rs"));
