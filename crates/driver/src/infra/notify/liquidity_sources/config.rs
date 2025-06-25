use std::time::Duration;

use derive_more::Debug;
use crate::domain::eth;

#[derive(Debug, Clone)]
pub struct Config {
    pub liquorice: Option<Liquorice>
}

#[derive(Debug, Clone)]
pub struct Liquorice {
    /// The URL to post notifications to
    pub notification_url: String,
    /// API key for the Liquorice API
    #[debug(ignore)]
    pub api_key: String,
    /// The HTTP timeout for requests to the Liquorice API
    pub http_timeout: Duration,
    /// The address of the Liquorice settlement contract to detect
    /// relevant interactions
    pub settlement_contract: eth::ContractAddress,
}
