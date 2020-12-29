#[cfg(feature = "bin")]
pub mod paths;

include!(concat!(env!("OUT_DIR"), "/IERC20.rs"));
include!(concat!(env!("OUT_DIR"), "/ERC20Mintable.rs"));
include!(concat!(env!("OUT_DIR"), "/UniswapV2Pair.rs"));
include!(concat!(env!("OUT_DIR"), "/UniswapV2Router02.rs"));
include!(concat!(env!("OUT_DIR"), "/UniswapV2Factory.rs"));
include!(concat!(env!("OUT_DIR"), "/GPv2Settlement.rs"));
include!(concat!(env!("OUT_DIR"), "/GPv2AllowListAuthentication.rs"));
