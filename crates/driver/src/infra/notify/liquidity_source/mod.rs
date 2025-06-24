pub mod config;

pub use config::Config;
use crate::infra;

/// Auctions and settlement notifier for liquidity sources
pub struct Notifier {
}

impl Notifier {
    pub fn new(_config: &infra::notify::liquidity_source::Config) -> Self {
        Self {}
    }

    /// Sends notification to liquidity sources before settlement
    pub async fn notify_before_settlement(&self) {
        todo!()
    }
}
