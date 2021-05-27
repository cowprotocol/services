pub mod amm_pair_provider;
pub mod arguments;
pub mod baseline_solver;
pub mod conversions;
pub mod current_block;
pub mod gas_price_estimation;
pub mod http;
#[macro_use]
pub mod macros;
pub mod bad_token;
pub mod balancer_event_handler;
pub mod ethcontract_error;
pub mod event_handling;
pub mod maintenance;
pub mod metrics;
pub mod network;
pub mod pool_aggregating;
pub mod pool_fetching;
pub mod price_estimate;
pub mod time;
pub mod token_info;
pub mod token_list;
pub mod trace_many;
pub mod tracing;
pub mod transport;

pub type Web3 = web3::Web3<transport::LoggingTransport<web3::transports::Http>>;
